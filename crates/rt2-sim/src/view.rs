//! Read-only views of a [`World`] for front ends (HUD, saving), so they never clone the save.

use rt2_formats::map::Save;

use crate::world::World;

impl World {
    /// The save the world was built from, with the state the simulation has written back into
    /// it so far (the clock fields are only synchronised by [`World::to_save`]). `None` for a
    /// world that was not built with [`World::from_save`].
    pub fn save_ref(&self) -> Option<&Save> {
        self.sim.as_ref().map(|s| &s.save)
    }
}
