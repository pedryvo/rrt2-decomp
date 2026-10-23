//! Slot-pool helpers over the parsed save pools (docs/spec/world-tick.md section 1,
//! docs/formats/map-gam.md 5.3). Every list of the simulation is a [`Pool`]: records addressed by
//! slot, a free bitmap and, for the linked ones, `next`/`prev` arrays. The simulation walks the
//! linked pools in list order (never in slot order) and keeps the arrays as the save has them.

use rt2_formats::map::{Pool, PoolRecord};

/// Index of the record of `slot` in `pool.records` (kept sorted by slot).
fn index_of(pool: &Pool, slot: usize) -> Option<usize> {
    pool.records.binary_search_by_key(&slot, |r| r.slot).ok()
}

/// `list_record` (`0x4f18b0`): the record of `slot`; slot 0 is null.
pub fn record(pool: &Pool, slot: usize) -> Option<&PoolRecord> {
    if slot == 0 {
        return None;
    }
    index_of(pool, slot).map(|i| &pool.records[i])
}

pub fn record_mut(pool: &mut Pool, slot: usize) -> Option<&mut PoolRecord> {
    if slot == 0 {
        return None;
    }
    index_of(pool, slot).map(|i| &mut pool.records[i])
}

/// Slot of the first record in list order (`pool_nth_id_list_order(list, 0)`, `0x4f18d0`).
pub fn first(pool: &Pool) -> usize {
    if pool.count == 0 {
        return 0;
    }
    if pool.linked {
        usize::from(pool.head)
    } else {
        pool.records.first().map_or(0, |r| r.slot)
    }
}

/// `pool_next_id` (`0x4f1960`): the slot after `slot` in list order, 0 at the end. An unlinked
/// pool is walked in slot order (`0x4f1e30`).
pub fn next(pool: &Pool, slot: usize) -> usize {
    if pool.linked {
        pool.next.get(slot).map_or(0, |&n| usize::from(n))
    } else {
        pool.records
            .iter()
            .find(|r| r.slot > slot)
            .map_or(0, |r| r.slot)
    }
}

/// Slots in list order (a snapshot; callers that must follow the live list, like pattern A of
/// world-tick.md section 1, walk with [`first`] and [`next`] instead).
pub fn list_order(pool: &Pool) -> Vec<usize> {
    let mut out = Vec::with_capacity(pool.count);
    let mut cur = first(pool);
    while cur != 0 && out.len() <= pool.records.len() {
        out.push(cur);
        cur = next(pool, cur);
    }
    out
}

/// `pool_nth_list_order` (`0x4f19e0`, pattern C of world-tick.md section 1): the n-th slot of
/// the list.
pub fn nth(pool: &Pool, n: usize) -> usize {
    list_order(pool).get(n).copied().unwrap_or(0)
}

/// Why [`alloc`] gave up (kept for callers that test it; the pool grows instead).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolFull;

fn is_free(pool: &Pool, s: usize) -> bool {
    s < pool.slots && pool.bitmap[s >> 3] & (1 << (s & 7)) != 0
}

/// `pool_grow` (`0x4f14c0`): adds `by` slots. The free bitmap keeps its old bytes (bits past the
/// old end included) and the new bytes are `0xff`; the original leaves the new `next`/`prev`
/// entries uninitialized: under Wine they read `0xBAADF00D` (the debug heap fill, byte `i` of the
/// array is `[0x0d, 0xf0, 0xad, 0xba][i % 4]`), which the snapshots of the original show.
pub fn grow(pool: &mut Pool, by: usize) {
    let old = pool.slots;
    let keep = if old == 0 { 0 } else { (old + 8) >> 3 };
    let size = (old + 8 + by) >> 3;
    pool.bitmap.truncate(keep);
    pool.bitmap.resize(size, 0xff);
    if pool.linked {
        let fill = |j: usize| {
            const PAT: [u8; 4] = [0x0d, 0xf0, 0xad, 0xba];
            u16::from_le_bytes([PAT[(2 * j) % 4], PAT[(2 * j + 1) % 4]])
        };
        pool.next.extend((old..old + by).map(fill));
        pool.prev.extend((old..old + by).map(fill));
    }
    pool.slots = old + by;
}

/// `pool_alloc` (`0x4f1690`, the path taken in a local game): scans the free bitmap upward from
/// the last allocated slot (`field_04`), wraps to slot 1 and scans up to `field_04`; when no slot
/// is free the pool grows by its step (`field_0c`) and the first new slot is used. The record is
/// linked at the end of the list.
pub fn alloc(pool: &mut Pool, data: Vec<u8>) -> Result<usize, PoolFull> {
    let n = pool.slots;
    let cur = pool.field_04 as i64;
    let mut found = None;
    let mut u = cur;
    loop {
        u += 1;
        if u >= n as i64 {
            if cur >= 2 {
                found = (1..cur)
                    .find(|&s| is_free(pool, s as usize))
                    .map(|s| s as usize);
            }
            break;
        }
        if is_free(pool, u as usize) {
            found = Some(u as usize);
            break;
        }
    }
    let slot = match found {
        Some(s) => s,
        None => {
            let step = pool.field_0c.max(1) as usize;
            grow(pool, step);
            n
        }
    };
    let at = pool.records.partition_point(|r| r.slot < slot);
    pool.records.insert(at, PoolRecord { slot, data });
    if pool.linked {
        if pool.count == 0 {
            pool.prev[slot] = 0;
            pool.head = slot as u16;
            pool.tail = slot as u16;
        } else {
            let tail = usize::from(pool.tail);
            pool.next[tail] = slot as u16;
            pool.prev[slot] = tail as u16;
            pool.tail = slot as u16;
        }
        pool.next[slot] = 0;
    }
    pool.field_04 = slot as i32;
    pool.bitmap[slot >> 3] &= !(1 << (slot & 7));
    pool.count += 1;
    refresh_order(pool);
    Ok(slot)
}

/// `pool_free` (`0x4f17e0`): unlinks the slot and marks it free. The count drops only if the slot
/// was live; the freed slot keeps its stale `next`/`prev` entries, as in the original.
pub fn free(pool: &mut Pool, slot: usize) {
    if is_free(pool, slot) || slot >= pool.slots {
        if slot < pool.slots {
            pool.bitmap[slot >> 3] |= 1 << (slot & 7);
        }
        return;
    }
    pool.count -= 1;
    if pool.linked {
        let p = pool.prev[slot];
        let nx = pool.next[slot];
        if p != 0 {
            pool.next[usize::from(p)] = nx;
        }
        if nx != 0 {
            pool.prev[usize::from(nx)] = p;
        }
        if usize::from(pool.head) == slot {
            pool.head = nx;
        }
        if usize::from(pool.tail) == slot {
            pool.tail = p;
        }
    }
    pool.bitmap[slot >> 3] |= 1 << (slot & 7);
    if let Some(i) = index_of(pool, slot) {
        pool.records.remove(i);
    }
    refresh_order(pool);
}

fn refresh_order(pool: &mut Pool) {
    pool.order = if pool.linked {
        list_order(pool)
    } else {
        pool.records.iter().map(|r| r.slot).collect()
    };
}

pub fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

pub fn i16_at(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}

pub fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

pub fn i32_at(b: &[u8], o: usize) -> i32 {
    u32_at(b, o) as i32
}

pub fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_bits(u32_at(b, o))
}

pub fn f64_at(b: &[u8], o: usize) -> f64 {
    f64::from_le_bytes(b[o..o + 8].try_into().expect("8 bytes"))
}

pub fn put_u16(b: &mut [u8], o: usize, v: u16) {
    b[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

pub fn put_i16(b: &mut [u8], o: usize, v: i16) {
    b[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

pub fn put_u32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

pub fn put_f32(b: &mut [u8], o: usize, v: f32) {
    put_u32(b, o, v.to_bits());
}

pub fn put_f64(b: &mut [u8], o: usize, v: f64) {
    b[o..o + 8].copy_from_slice(&v.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(slots: usize, linked: bool) -> Pool {
        let mut bitmap = vec![0xFFu8; (slots + 8) >> 3];
        bitmap[0] &= !1; // slot 0 is never free
        Pool {
            slots,
            record_size: 2,
            linked,
            field_00: 0,
            field_04: 0,
            field_0c: 0,
            count: 0,
            pool_type: 0,
            head: 0,
            tail: 0,
            records: Vec::new(),
            order: Vec::new(),
            bitmap,
            next: if linked { vec![0; slots] } else { Vec::new() },
            prev: if linked { vec![0; slots] } else { Vec::new() },
        }
    }

    #[test]
    fn alloc_scans_up_from_the_cursor_and_links_at_the_end() {
        let mut p = pool(6, true);
        assert_eq!(alloc(&mut p, vec![1, 1]), Ok(1));
        assert_eq!(alloc(&mut p, vec![2, 2]), Ok(2));
        assert_eq!(alloc(&mut p, vec![3, 3]), Ok(3));
        assert_eq!((p.count, p.field_04, p.head, p.tail), (3, 3, 1, 3));
        assert_eq!(list_order(&p), [1, 2, 3]);
        assert_eq!(p.prev[3], 2);
        // Free slot 1 by hand: the next allocation does not reuse it until the scan wraps.
        p.bitmap[0] |= 1 << 1;
        p.records.remove(0);
        assert_eq!(alloc(&mut p, vec![4, 4]), Ok(4));
        assert_eq!(alloc(&mut p, vec![5, 5]), Ok(5));
        assert_eq!(
            alloc(&mut p, vec![6, 6]),
            Ok(1),
            "wraps to the lowest free slot"
        );
        p.field_0c = 4;
        assert_eq!(alloc(&mut p, vec![7, 7]), Ok(6), "grows by the step");
        assert_eq!(p.slots, 10);
        let slots: Vec<usize> = p.records.iter().map(|r| r.slot).collect();
        assert_eq!(slots, [1, 2, 3, 4, 5, 6]);
        let before = p.count;
        free(&mut p, 3);
        assert_eq!(p.count, before - 1);
        assert_eq!(p.next[2], 4);
        assert_eq!(p.prev[4], 2);
    }

    #[test]
    fn unlinked_pool_is_walked_in_slot_order() {
        let mut p = pool(5, false);
        for v in 1..=3u8 {
            alloc(&mut p, vec![v, v]).unwrap();
        }
        assert_eq!(first(&p), 1);
        assert_eq!(next(&p, 1), 2);
        assert_eq!(next(&p, 3), 0);
        assert_eq!(nth(&p, 2), 3);
        assert!(record(&p, 0).is_none());
    }
}
