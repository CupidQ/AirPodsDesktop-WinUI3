//! Preserve user adjustments when restoring conversation ducking.
#[derive(Clone, Copy)]
pub struct DuckedVolume {
    original: f32,
    applied: f32,
}

impl DuckedVolume {
    pub fn new(original: f32) -> Self {
        Self {
            original,
            applied: original * 0.3,
        }
    }
    pub fn applied(self) -> f32 {
        self.applied
    }
    pub fn restore(self, current: f32) -> Option<f32> {
        ((current - self.applied).abs() < 0.0001).then_some(self.original)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restores_only_when_user_has_not_changed_volume() {
        let v = DuckedVolume::new(0.8);
        assert_eq!(v.restore(v.applied()), Some(0.8));
        assert_eq!(v.restore(0.5), None);
        let muted = DuckedVolume::new(0.0);
        assert_eq!(muted.restore(0.0), Some(0.0));
    }
}
