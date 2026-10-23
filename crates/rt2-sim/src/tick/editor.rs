//! The editor's operations that need the simulation state (`SimState` tables and the train-step
//! view): the ground blends (`0x42a7d0`), buildings and industries (command `0x17`, shapes
//! `24..121`, `docs/spec/commands.md` 3.3), the bulldozer with building demolition, and the
//! recounts the saver relies on. The pure tile tools live in [`crate::editor`].

use super::*;
use crate::ai::Ai;
use crate::build::cmd::CmdError;
use crate::build::station::NewBuilding;
use crate::editor::EditorError;
use crate::fields;
use crate::rng::GameRng;
use crate::tilemark;

impl SimState {
    /// `0x42a7d0(x, y, w, h)`: bakes the ground blends and shores of a rectangle into the terrain
    /// ids (the saved `.MP2` files keep them) and refreshes the layer cache.
    pub(crate) fn editor_blend(&mut self, rng: &mut GameRng, r: [i32; 4]) {
        let cached = self.track_cache.take();
        let local;
        let track = match &cached {
            Some(t) => t,
            None => {
                local = self.track_mask(r[0] - 6, r[1] - 6, r[0] + r[2] + 6, r[1] + r[3] + 6);
                &local
            }
        };
        let mut ground = self.take_ground_layer();
        let mut ctx = fields::FieldCtx {
            dirty: &mut self.dirty,
            ground: &mut ground,
            track,
        };
        fields::ground_rect(&mut self.save, &mut ctx, rng, r[0], r[1], r[2], r[3]);
        self.ground_layer = Some(ground);
        self.track_cache = cached;
    }

    /// Command `0x17` as the editor sends it: the placement test (`0x4087a0`) and
    /// `building_create` (`0x409010`) with owner 0, then what the shared primitive leaves to its
    /// callers for a business or house: the field textures (`0x409120`, `0x429fc0`, `0x42a3e0`) and
    /// the station links (`0x4072c0`). `style` is the architecture hint (`-1` = the town's).
    fn editor_build(
        &mut self,
        w: &mut World,
        x: i32,
        y: i32,
        shape: i32,
        style: i32,
    ) -> Result<i32, EditorError> {
        let id = self.with_trainstep(w, |ts| {
            let mut ai = Ai::new(ts);
            if ai
                .building_can_place(x, y, shape, 0, true, 0, true, false)
                .is_none()
            {
                return Err(EditorError::InvalidPlacement);
            }
            let id = ai.building_create(NewBuilding {
                x,
                y,
                shape,
                ghost: false,
                company: 0,
                place: 0,
                hint: style,
            });
            ai.flush_redraw();
            Ok(id)
        })?;
        let slot = id as usize;
        let img = &self.data.image;
        let kind = tilemark::shape_type(img, shape as usize);
        if kind != 0 {
            let origin = pool::record(&self.save.objects, slot).map_or((x, y), |r| {
                (
                    i32::from(i16_at(&r.data, 0x1e)),
                    i32::from(i16_at(&r.data, 0x20)),
                )
            });
            if tilemark::cells()
                .any(|(l, c)| tilemark::shape_cell(img, shape as usize, l, c) & 0x40 != 0)
            {
                let cached = self.track_cache.take();
                let local;
                let track = match &cached {
                    Some(t) => t,
                    None => {
                        local = self.track_mask(
                            origin.0 - 12,
                            origin.1 - 12,
                            origin.0 + 5,
                            origin.1 + 5,
                        );
                        &local
                    }
                };
                let mut ground = self.take_ground_layer();
                let img = &self.data.image;
                let mut ctx = fields::FieldCtx {
                    dirty: &mut self.dirty,
                    ground: &mut ground,
                    track,
                };
                let head = fields::paint_fields(
                    &mut self.save,
                    &mut ctx,
                    img,
                    &mut w.rng,
                    shape as usize,
                    kind,
                    origin,
                    0,
                );
                self.ground_layer = Some(ground);
                self.track_cache = cached;
                if let Some(r) = pool::record_mut(&mut self.save.objects, slot) {
                    put_u16(&mut r.data, 0x50, head);
                }
            }
            self.link_building(slot);
        }
        Ok(id)
    }

    /// Bulldozer (command `0x0a`, owner 0): track, trees and buildings in the radius. The
    /// shared primitive prices and removes track and trees; the buildings it leaves to
    /// `0x408e70` (`building_destroy`, here [`SimState::building_destroy`]) are taken down
    /// afterwards, once per building.
    fn editor_bulldoze(
        &mut self,
        w: &mut World,
        x: i32,
        y: i32,
        radius: i32,
    ) -> Result<i32, EditorError> {
        let (r, buildings) = self.with_trainstep(w, |ts| {
            let mut ai = Ai::new(ts);
            let (mw, mh) = (ai.ts.map_w(), ai.ts.map_h());
            let mut found: Vec<i32> = Vec::new();
            for tx in x - radius..=x + radius {
                for ty in y - radius..=y + radius {
                    if tx < 0
                        || ty < 0
                        || tx >= mw
                        || ty >= mh
                        || crate::track::tile_distance(tx, ty, x, y) > radius
                    {
                        continue;
                    }
                    let b = ai.building_at(tx, ty);
                    if b != 0 && !found.contains(&b) {
                        found.push(b);
                    }
                }
            }
            (ai.bulldoze_area(0, x, y, radius, false), found)
        });
        let cost = r.map_err(|e| EditorError::Command(CmdError::Bulldoze(e)))?;
        for b in buildings {
            if pool::record(&self.save.objects, b as usize).is_some() {
                self.building_destroy(&mut w.rng, b as usize);
            }
        }
        Ok(cost)
    }

    /// What the loader does after the passes and the saver relies on: each place's box and land
    /// tiles (`0x430e70`) and each territory's land count (`0x4c8970`).
    pub(crate) fn editor_recompute(&mut self) {
        self.towns_recompute_extent();
        self.territories_recount_land();
    }
}

fn i16_at(b: &[u8], off: usize) -> i16 {
    i16::from_le_bytes([b[off], b[off + 1]])
}

impl World {
    /// Bakes the ground blends of the rectangle `(x, y, w, h)` into the terrain ids (`0x42a7d0`).
    pub fn editor_blend(&mut self, r: [i32; 4]) {
        let Some(mut sim) = self.sim.take() else {
            return;
        };
        sim.editor_blend(&mut self.rng, r);
        self.sim = Some(sim);
    }

    /// Places a building, industry, house or other shape `24..121` at the tile, with owner 0
    /// (tool 7, command `0x17`). Returns the new building's id.
    pub fn editor_build(
        &mut self,
        x: i32,
        y: i32,
        shape: i32,
        style: i32,
    ) -> Result<i32, EditorError> {
        let Some(mut sim) = self.sim.take() else {
            return Err(EditorError::NoMap);
        };
        let r = sim.editor_build(self, x, y, shape, style);
        self.sim = Some(sim);
        r
    }

    /// Whether a shape could be placed at the tile (the test the click makes first).
    pub fn editor_can_build(&mut self, x: i32, y: i32, shape: i32) -> bool {
        let Some(mut sim) = self.sim.take() else {
            return false;
        };
        let ok = sim.with_trainstep(self, |ts| {
            let ai = Ai::new(ts);
            ai.building_can_place(x, y, shape, 0, true, 0, true, false)
                .is_some()
        });
        self.sim = Some(sim);
        ok
    }

    /// The bulldozer (tool 8) over a radius.
    pub fn editor_bulldoze(&mut self, x: i32, y: i32, radius: i32) -> Result<i32, EditorError> {
        let Some(mut sim) = self.sim.take() else {
            return Err(EditorError::NoMap);
        };
        let r = sim.editor_bulldoze(self, x, y, radius);
        self.sim = Some(sim);
        r
    }

    /// Recomputes the place boxes and territory land counts (before saving).
    pub fn editor_recompute(&mut self) {
        if let Some(sim) = self.sim.as_mut() {
            sim.editor_recompute();
        }
    }
}
