//! Route extension by the pathfinder (trains.md 10.7): `train_route_extend` (`0x4cf8c0`) and
//! `track_pathfind` (`0x460850`) in train mode (`param_7 == 0`, along existing track).
//!
//! The search is a uniform-cost search from the route's last tile to the stop's tile over the
//! track graph, with a bucket queue (`0x4fa140`: buckets of width 4, each a list sorted by cost,
//! a new entry goes before the first one of equal or higher cost) and per-tile, per-direction
//! best costs (`0x462690`). Stale queue entries are expanded again, as in the original.

use super::geom::{DX, DY};
use super::mem::{P, Ts};

/// One queue entry (`0x4fa140` record, 13 bytes: u32 cost, u16 next, u16 x, u16 y, u8 dir,
/// u8, u8).
#[derive(Clone, Copy, Debug)]
struct Entry {
    cost: u32,
    x: i32,
    y: i32,
    dir: u8,
}

/// The bucket queue of `0x4fa140` (width 4).
#[derive(Default)]
struct Buckets {
    lists: Vec<Vec<Entry>>,
}

impl Buckets {
    const WIDTH: u32 = 4;

    /// `0x4fa490`: inserts before the first entry whose cost is `>=` the new one.
    fn push(&mut self, e: Entry) {
        let b = (e.cost / Self::WIDTH) as usize;
        if self.lists.len() <= b {
            self.lists.resize_with(b + 1, Vec::new);
        }
        let l = &mut self.lists[b];
        let at = l.iter().position(|o| e.cost <= o.cost).unwrap_or(l.len());
        l.insert(at, e);
    }

    /// `0x4fa770` + `0x4fa6c0`: the head of the first non-empty bucket at or after the bucket of
    /// `from`.
    fn pop(&mut self, from: u32) -> Entry {
        let mut b = (from as i32 / Self::WIDTH as i32) as usize;
        while self.lists.get(b).is_none_or(|l| l.is_empty()) {
            b += 1;
        }
        self.lists[b].remove(0)
    }
}

/// Per-tile search node (`0x566a74` records, 0x2a bytes): best cost and parent direction per
/// entry direction, and the mask of directions seen.
#[derive(Clone, Copy, Default)]
struct Node {
    cost: [u32; 8],
    back: [u8; 8],
    mask: u8,
}

struct Search {
    nodes: std::collections::HashMap<(i32, i32), Node>,
    queue: Buckets,
    count: i32,
    /// `0x566a38/3c/2c/30`: search box.
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    /// `0x566a48`: cost limit.
    limit: i32,
}

impl Search {
    /// `0x462690` (train mode: the two extra bytes are 0): relax `(x, y)` entered with `dir`.
    fn push(&mut self, x: i32, y: i32, dir: u8, back: u8, cost: u32) {
        if x < self.x0 || x > self.x1 || y < self.y0 || y > self.y1 || cost as i32 > self.limit {
            return;
        }
        let n = self.nodes.entry((x, y)).or_default();
        let bit = 1u8 << dir;
        if n.mask & bit != 0 && n.cost[usize::from(dir)] as i32 <= cost as i32 {
            return;
        }
        n.mask |= bit;
        n.cost[usize::from(dir)] = cost;
        n.back[usize::from(dir)] = back;
        self.queue.push(Entry { cost, x, y, dir });
        self.count += 1;
    }
}

/// Result of a train-mode search: the path from the destination back to the source as
/// `(x, y, dir_a, dir_b)` (arrays `X, Y, A, B` of `0x460850`).
struct Path {
    x: Vec<i32>,
    y: Vec<i32>,
    a: Vec<u8>,
    b: Vec<u8>,
}

impl Ts<'_> {
    /// `train_route_extend` (`0x4cf8c0`): appends the path to the current stop to the route
    /// ring buffer; when the train already stands on its stop, moves to the next stop first
    /// (`0x4d1630`). `false` when no path was found (`+0x92 = 1`).
    pub fn route_extend(&mut self, t: i32) -> bool {
        let nstops = i32::from(self.u8(P::Train, t, 0x3b));
        if nstops == 0 {
            return false;
        }
        if self.u8(P::Train, t, 0x92) != 0 && self.u8(P::Train, t, 0x93) == 0 {
            return false;
        }
        let has_route = self.i32(P::Train, t, 0x65) != 0 && self.i32(P::Train, t, 0x61) >= 1;
        let room = if has_route {
            let r = 0x20 - self.i32(P::Train, t, 0x61);
            if r < 1 {
                return false;
            }
            r
        } else {
            0x20
        };
        let (sx, sy) = if has_route {
            let h = self.i32(P::Train, t, 0x5d);
            (self.rx(t, h), self.ry(t, h))
        } else {
            let head = self.i32(P::Train, t, 0);
            (
                i32::from(self.i16(P::Car, head, 9)),
                i32::from(self.i16(P::Car, head, 0xb)),
            )
        };
        let mut tries = 0;
        loop {
            let k = self.cur_stop(t);
            let (dx, dy) = self.stop_xy(t, k);
            if sx != dx || sy != dy {
                let route65 = self.i32(P::Train, t, 0x65) != 0;
                let h = self.i32(P::Train, t, 0x5d);
                let rev_start = route65 && {
                    let f = self.rflags(t, h);
                    f & 1 != 0 && f & 8 != 0
                };
                let start_dir = if route65 {
                    i32::from(self.rdin(t, h))
                } else {
                    -1
                };
                let head = self.i32(P::Train, t, 0);
                let electric = self.en8(self.vt_engine(self.u8(P::Car, head, 8)), 0x28) == 2;
                let company = self.i32(P::Train, t, 0x41);
                if let Some(len) = self.pathfind_train(
                    t,
                    (sx, sy),
                    (dx, dy),
                    room,
                    route65,
                    start_dir,
                    company,
                    rev_start,
                    electric,
                ) {
                    let v = self.i16(P::Train, t, 0x7d).wrapping_add(len as i16);
                    self.set16(P::Train, t, 0x7b, v as u16);
                    self.set8(P::Train, t, 0x92, 0);
                    self.set8(P::Train, t, 0x93, 0);
                    return true;
                }
                self.set8(P::Train, t, 0x92, 1);
                self.set8(P::Train, t, 0x93, 0);
                return false;
            }
            self.set_stop(t, (k + 1) % nstops, false);
            tries += 1;
            if nstops < tries {
                return false;
            }
        }
    }

    /// `train_set_stop` (`0x4d1630`, `this, k, refresh`): makes stop `k` (0 when out of range)
    /// the current one and resets the per-leg counters.
    pub fn set_stop(&mut self, t: i32, k: i32, refresh: bool) {
        let k = if i32::from(self.u8(P::Train, t, 0x3b)) <= k {
            0
        } else {
            k
        };
        if self.cur_stop(t) == k {
            return;
        }
        let head = self.i32(P::Train, t, 0);
        if self.u8(P::Car, head, 0x2a) != 0 && self.u8(P::Train, t, 0x9a) != 3 {
            self.seti32(P::Train, t, 0x71, 0);
        }
        self.set8(P::Train, t, 0x3c, k as u8);
        self.set16(P::Train, t, 0x7b, 0);
        self.set16(P::Train, t, 0x7d, 0);
        self.set16(P::Train, t, 0x7f, 0);
        self.set8(P::Train, t, 0x81, 0);
        self.set16(P::Train, t, 0xcb, 0xffff);
        if self.u8(P::Car, head, 0x2a) != 0 {
            let (x, y) = (
                i32::from(self.i16(P::Car, head, 9)),
                i32::from(self.i16(P::Car, head, 0xb)),
            );
            self.enter_tile(t, x, y);
        }
        self.set8(P::Train, t, 0x93, 0);
        self.set8(P::Train, t, 0x92, 0);
        let mut c = head;
        while c != 0 {
            self.set16(P::Car, c, 0xd, 0xffff);
            c = i32::from(self.i16(P::Car, c, 2));
        }
        if refresh {
            self.train_route_refresh(t);
        }
        if self.flag_570229 != 0 {
            todo_hook!(
                self.log,
                "train_stop_cargo_570229",
                "0x4d6330 (stop cargo lists, flag 0x570229)",
                "trains.md 10.7"
            );
        }
        self.seti32(P::Train, t, 0x2f, 0);
    }

    /// Tile of stop `k`: the waypoint's `+0xa`/`+0xc` (i16) or the station's `+0xa`/`+0xe`.
    fn stop_xy(&self, t: i32, k: i32) -> (i32, i32) {
        if self.stop8(t, k, 3) & 8 != 0 {
            (
                i32::from(self.stop16(t, k, 0xa) as i16),
                i32::from(self.stop16(t, k, 0xc) as i16),
            )
        } else {
            let st = i32::from(self.stop16(t, k, 0));
            (self.i32(P::Station, st, 0xa), self.i32(P::Station, st, 0xe))
        }
    }

    /// `track_path_cost` (`0x463b30`) in train mode: 2 (orthogonal) or 3 (diagonal) on the
    /// company's track, 6 or 9 on foreign track, x3 for a reversal (`d == opposite(d_in)`).
    fn path_step_cost(&self, x: i32, y: i32, d: u8, d_in: u8, company: i32) -> u32 {
        let p = self.tile_track(x, y);
        let own = company == 0 || i32::from(self.u8(P::Track, p, 2)) == company;
        let c = if own {
            u32::from(d & 1) | 2
        } else if d & 1 != 0 {
            9
        } else {
            6
        };
        if d != d_in.wrapping_sub(4) & 7 {
            c
        } else {
            c * 3
        }
    }

    /// `track_pathfind` (`0x460850`) with the arguments of `0x4cf8c0` (train mode). Writes the
    /// path into the route ring of `t` and returns its length (`*param_9`), or `None`.
    #[allow(clippy::too_many_arguments)]
    fn pathfind_train(
        &mut self,
        t: i32,
        src: (i32, i32),
        dst: (i32, i32),
        room: i32,
        route65: bool,
        start_dir: i32,
        company: i32,
        rev_start: bool,
        electric: bool,
    ) -> Option<i32> {
        let (w, h) = (self.map_w(), self.map_h());
        let in_map = |(x, y): (i32, i32)| x >= 0 && x < w && y >= 0 && y < h;
        if !(in_map(src) && in_map(dst)) {
            return None;
        }
        let elec_ok = |ts: &Ts<'_>, (x, y): (i32, i32)| {
            let p = ts.tile_track(x, y);
            p != 0 && ts.u8(P::Track, p, 0x23) & 0x10 != 0
        };
        if electric && !(elec_ok(self, src) && elec_ok(self, dst)) {
            return None;
        }
        let owner = company;
        // 0x566a58: the company may only use its own territories (company +0x422e == 0).
        let restricted = company != 0 && self.u8(P::Company, company, 0x422e) == 0;
        if restricted {
            let ok = |ts: &Ts<'_>, (x, y): (i32, i32)| {
                let terr = usize::from(ts.t8(x, y, 9));
                ts.u8(P::Company, company, 0x479 + terr * 0x1ed) & 1 != 0 || ts.tile_is_water(x, y)
            };
            if !ok(self, src) || !ok(self, dst) {
                return None;
            }
        }
        let st = &self.s.state;
        let g = |o: usize| i32::from_le_bytes([st[o], st[o + 1], st[o + 2], st[o + 3]]);
        // 0x573d7e/82/86/8a (state +0x3896..): the playable rectangle; inside it the search keeps
        // three tiles away from the map edge.
        let margin = if g(0x3896) > 2 && g(0x389a) > 2 && g(0x389e) <= w - 4 && g(0x38a2) <= h - 4 {
            3
        } else {
            0
        };
        let mut s = Search {
            nodes: Default::default(),
            queue: Buckets::default(),
            count: 0,
            x0: margin,
            y0: margin,
            x1: w - 1 - margin,
            y1: h - 1 - margin,
            limit: 999_999_999,
        };
        let (sx, sy) = src;
        if start_dir == -1 {
            let mask = self.piece_mask(sx, sy);
            for d in 0..8u8 {
                if mask & (1 << d) != 0 {
                    let c = self.path_step_cost(sx, sy, d, d, owner);
                    s.push(
                        sx + DX[usize::from(d)],
                        sy + DY[usize::from(d)],
                        d.wrapping_sub(4) & 7,
                        0,
                        c + 1,
                    );
                }
            }
        } else {
            s.push(sx, sy, start_dir as u8, 0, 1);
        }
        let mut last = 0u32;
        let found = loop {
            if s.count < 1 {
                break None;
            }
            s.count -= 1;
            let e = s.queue.pop(last);
            last = e.cost;
            if (e.x, e.y) == dst {
                break Some(e);
            }
            let (x, y) = (e.x, e.y);
            if electric && !elec_ok(self, (x, y)) {
                continue;
            }
            if restricted {
                let terr = usize::from(self.t8(x, y, 9));
                if self.u8(P::Company, company, 0x479 + terr * 0x1ed) & 1 == 0
                    && !self.tile_is_water(x, y)
                {
                    continue;
                }
            }
            let mask = self.piece_mask(x, y);
            let d = e.dir;
            let ahead = d.wrapping_sub(4) & 7;
            let relax = |ts: &Ts<'_>, s: &mut Search, nd: u8| {
                if mask & (1 << nd) != 0 {
                    let c = ts.path_step_cost(x, y, nd, ahead, owner);
                    s.push(
                        x + DX[usize::from(nd)],
                        y + DY[usize::from(nd)],
                        nd.wrapping_sub(4) & 7,
                        d,
                        c + e.cost,
                    );
                }
            };
            relax(self, &mut s, d.wrapping_add(3) & 7);
            relax(self, &mut s, ahead);
            relax(self, &mut s, d.wrapping_sub(3) & 7);
            relax(self, &mut s, d);
        };
        let e = found?;
        // Path reconstruction (0x461a63..): from the destination back to the source.
        let mut p = Path {
            x: Vec::new(),
            y: Vec::new(),
            a: Vec::new(),
            b: Vec::new(),
        };
        let (mut x, mut y) = (e.x, e.y);
        let mut d = e.dir;
        let mut prev = d;
        let mut back = s.nodes[&(x, y)].back[usize::from(d)];
        loop {
            let b_k = back;
            p.x.push(x);
            p.y.push(y);
            p.a.push(d);
            p.b.push(prev.wrapping_sub(4) & 7);
            prev = d;
            x += DX[usize::from(d)];
            y += DY[usize::from(d)];
            let sd = start_dir;
            let ok_dir = rev_start
                || sd == -1
                || [0i32, 1, -1, 3, -4, -3]
                    .iter()
                    .any(|k| sd == (i32::from(d) + k) & 7);
            if (x, y) == src && ok_dir {
                break;
            }
            back = s.nodes[&(x, y)].back[usize::from(b_k)];
            d = b_k;
        }
        // The source entry: the original's `bVar4` is the direction of the last node walked
        // (`d`, towards the source), not its parent's (`0x461a63`..`0x462260`, snapshots tp6).
        p.x.push(x);
        p.y.push(y);
        p.b.push(d.wrapping_sub(4) & 7);
        p.a.push(d);
        let cnt = p.x.len() as i32;
        let len = if route65 && !rev_start { cnt - 1 } else { cnt };
        self.write_route(t, &p, len, room, rev_start, route65);
        Some(len)
    }

    pub(crate) fn piece_mask(&self, x: i32, y: i32) -> u8 {
        let p = self.tile_track(x, y);
        if p == 0 { 0 } else { self.u8(P::Track, p, 0) }
    }

    /// `terrain_class_of_id[tile >> 6] == 0x12` (water). The class table (`0x581090`) is built
    /// at start-up (`0x471b10`); the two water ground ids of the files are used instead.
    pub fn tile_is_water(&self, x: i32, y: i32) -> bool {
        let g = self.t8(x, y, 1);
        g == 0x61 || g == 0x62
    }

    /// `0x462830(route, n)`: allocates the ring of a train (`+0x45..+0x55`, size `+0x59 = n`,
    /// head `+0x5d = -1`, count `+0x61 = 0`, `+0x65 = 1`). The arrays are `malloc` memory in
    /// the original (uninitialized, see below).
    pub(crate) fn route_init(&mut self, t: i32, n: i32) {
        let e = self.extra_mut_pub(t);
        // The original's `malloc` memory reads `0xBAADF00D` (the Wine debug heap fill) in
        // every array; the arrays are `x` (2n), `y` (2n), `din` (n), `dout` (n), flags (2n).
        let fill = |len: usize| (0..len).map(|i| [0x0d, 0xf0, 0xad, 0xba][i % 4]);
        let n = n as usize;
        e.route_raw = [2 * n, 2 * n, n, n, 2 * n]
            .into_iter()
            .flat_map(fill)
            .collect();
        let n = n as i32;
        self.seti32(P::Train, t, 0x59, n);
        self.seti32(P::Train, t, 0x5d, -1);
        self.seti32(P::Train, t, 0x61, 0);
        self.seti32(P::Train, t, 0x65, 1);
    }

    /// `0x4627f0`: frees the ring of a train (`+0x65 = 0`).
    pub(crate) fn route_free(&mut self, t: i32) {
        if self.i32(P::Train, t, 0x65) != 0 {
            self.extra_mut_pub(t).route_raw.clear();
            self.seti32(P::Train, t, 0x65, 0);
        }
    }

    /// The copy of the path into the route ring (`0x462260..0x4626a4`).
    fn write_route(
        &mut self,
        t: i32,
        p: &Path,
        len: i32,
        room: i32,
        rev_start: bool,
        route65: bool,
    ) {
        if self.i32(P::Train, t, 0x65) == 0 {
            // `0x460850` allocates the ring on first use (`0x462260`): 0x20 steps.
            self.route_init(t, 0x20);
        }
        let n59 = self.i32(P::Train, t, 0x59);
        let mut room = room;
        if room == 0 {
            return;
        }
        if n59 - self.i32(P::Train, t, 0x61) <= room {
            room = n59 - self.i32(P::Train, t, 0x61);
        }
        if room == 0 {
            return;
        }
        let at = |v: &Vec<i32>, i: i32| v[i as usize];
        let at8 = |v: &Vec<u8>, i: i32| v[i as usize];
        let mut written = 0;
        let mut k = 0;
        loop {
            let next_written = written + 1;
            let head = (self.i32(P::Train, t, 0x5d) + 1) % n59;
            let c61 = self.i32(P::Train, t, 0x61) + 1;
            self.seti32(P::Train, t, 0x61, c61);
            let i = len - k;
            self.seti32(P::Train, t, 0x5d, head);
            self.set_route_step(
                t,
                head,
                at(&p.x, i - 1) as u16,
                at(&p.y, i - 1) as u16,
                at8(&p.a, i - 1),
                at8(&p.b, i - 1),
            );
            // The original ORs the uninitialized build-mode arrays C and D into bits 4-7 and
            // 14-15 of the flags (heap garbage); they are left as they were here.
            let f = self.rflags(t, head) & 0xf6;
            self.set_rflags(t, head, f);
            let k1 = k + 1;
            written = next_written;
            if k1 == len {
                let f = self.rflags(t, head);
                self.set_rflags(t, head, f | 1 | 8);
            } else if k == 0 && rev_start {
                let f = self.rflags(t, head);
                self.set_rflags(t, head, f | 8);
            } else if self.i32(P::Train, t, 0x61) < 2 || len < k1 {
                let f = self.rflags(t, head);
                self.set_rflags(t, head, f & 0xfe);
            } else {
                let prev = (head - 1 + n59) % n59;
                if self.rx(t, prev) != at(&p.x, i - 2) || self.ry(t, prev) != at(&p.y, i - 2) {
                    let f = self.rflags(t, head);
                    self.set_rflags(t, head, f & 0xfe);
                } else {
                    let f = self.rflags(t, head);
                    self.set_rflags(t, head, f | 1);
                    let dout = self.rdout(t, head).wrapping_sub(4) & 7;
                    let (x, y, din) = (self.rx(t, head), self.ry(t, head), self.rdin(t, head));
                    self.set_route_step(t, head, x as u16, y as u16, din, dout);
                    if next_written < room {
                        let h2 = (self.i32(P::Train, t, 0x5d) + 1) % n59;
                        let c61 = self.i32(P::Train, t, 0x61) + 1;
                        self.seti32(P::Train, t, 0x61, c61);
                        self.seti32(P::Train, t, 0x5d, h2);
                        self.set_route_step(
                            t,
                            h2,
                            at(&p.x, i - 1) as u16,
                            at(&p.y, i - 1) as u16,
                            at8(&p.a, i - 1).wrapping_sub(4) & 7,
                            at8(&p.b, i - 1),
                        );
                        let f = self.rflags(t, h2);
                        self.set_rflags(t, h2, f & 0xfe);
                        written += 1;
                    }
                }
            }
            if !route65 && k == 0 {
                let h = self.i32(P::Train, t, 0x5d);
                let din = self.rdout(t, h).wrapping_sub(4) & 7;
                let (x, y, dout) = (self.rx(t, h), self.ry(t, h), self.rdout(t, h));
                self.set_route_step(t, h, x as u16, y as u16, din, dout);
                let f = self.rflags(t, h);
                self.set_rflags(t, h, f | 1);
            }
            if written == room {
                break;
            }
            k = k1;
            if k == len {
                break;
            }
        }
    }
}
