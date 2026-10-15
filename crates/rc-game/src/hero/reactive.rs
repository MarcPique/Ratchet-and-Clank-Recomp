//! The pvars of the three hand items that act on creatures through more than hits (docs/plan/hero_gameplay.md §10):
//! the Suck Cannon ([`super::suck_cannon`]), the Taunter ([`super::taunter`]) and the Morph-o-Ray. What they share in
//! the game is on the creatures' side ([`crate::moby_update::creature::react`]); each update is its own code.

/// [`super::weapons::Weapons::reactive`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reactive {
    pub suck: super::suck_cannon::SuckCannon,
    pub taunter: super::taunter::Taunter,
    pub morph: super::morph_ray::MorphRay,
}
