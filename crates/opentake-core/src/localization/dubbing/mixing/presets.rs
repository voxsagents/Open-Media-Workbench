//! Preset definitions and factories for audio mixing configurations.

use super::models::{DubbingMixConfiguration, MixPreset};

/// Applies preset values to a DubbingMixConfiguration while preserving project ID and custom track settings.
pub fn apply_preset_to_configuration(config: &mut DubbingMixConfiguration, preset: MixPreset) {
    config.preset = preset;
    match preset {
        MixPreset::Balanced => {
            config.original_audio_enabled = true;
            config.original_audio_gain_db = -6.0;
            config.dubbing_master_gain_db = 0.0;
            config.ducking_enabled = true;
            config.ducking_attenuation_db = -12.0;
            config.ducking_attack_ms = 50;
            config.ducking_release_ms = 250;
            config.ducking_hold_ms = 100;
            config.limiter_enabled = true;
            config.peak_ceiling_db = -1.0;
        }
        MixPreset::VoiceFocus => {
            config.original_audio_enabled = true;
            config.original_audio_gain_db = -12.0;
            config.dubbing_master_gain_db = 1.5;
            config.ducking_enabled = true;
            config.ducking_attenuation_db = -18.0;
            config.ducking_attack_ms = 30;
            config.ducking_release_ms = 300;
            config.ducking_hold_ms = 150;
            config.limiter_enabled = true;
            config.peak_ceiling_db = -1.0;
        }
        MixPreset::PreserveOriginal => {
            config.original_audio_enabled = true;
            config.original_audio_gain_db = 0.0;
            config.dubbing_master_gain_db = -1.0;
            config.ducking_enabled = true;
            config.ducking_attenuation_db = -6.0;
            config.ducking_attack_ms = 80;
            config.ducking_release_ms = 200;
            config.ducking_hold_ms = 80;
            config.limiter_enabled = true;
            config.peak_ceiling_db = -1.0;
        }
        MixPreset::Custom => {
            // Keep existing values unchanged
        }
    }
    config.touch();
}
