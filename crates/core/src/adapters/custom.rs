use std::path::Path;

use super::{detected, opt_str};
use crate::adapter::*;
use crate::env::{normalize_path, Env};
use crate::model::{InstallSource, Installation, Instance};

/// Any program the user points at. Without a known isolation switch all instances share the
/// app's data; the user can opt in to `--user-data-dir` when the program supports it.
pub struct CustomAdapter;

fn uses_data_dir(install: &Installation) -> bool {
    install.options.get("userDataDir").and_then(|v| v.as_bool()).unwrap_or(false)
}

impl AppAdapter for CustomAdapter {
    fn kind(&self) -> &'static str {
        "custom"
    }

    fn detect(&self, _env: &Env) -> Vec<Installation> {
        Vec::new()
    }

    fn recognize(&self, exe: &Path, _env: &Env) -> Option<Installation> {
        if !exe.is_file() {
            return None;
        }
        let stem = exe.file_stem()?.to_string_lossy().to_string();
        let mut i = detected(&format!("custom:{}", crate::model::new_id()), "custom", &stem, exe.to_path_buf());
        i.source = InstallSource::Manual;
        Some(i)
    }

    fn isolation(&self, install: &Installation) -> Isolation {
        if uses_data_dir(install) {
            Isolation::UserDataDir
        } else {
            Isolation::None
        }
    }

    fn launch_plan(&self, install: &Installation, instance: &Instance, _ctx: &Ctx) -> Result<LaunchPlan, String> {
        let mut args = if uses_data_dir(install) { user_data_dir_args(instance) } else { Vec::new() };
        args.extend(instance.launch.args.iter().cloned());
        Ok(LaunchPlan {
            program: install.exe.clone(),
            args,
            env: instance.launch.env.clone(),
            cwd: install.exe.parent().map(Path::to_path_buf),
        })
    }

    fn matcher(&self, install: &Installation, instance: &Instance, ctx: &Ctx) -> ProcessMatcher {
        let name = install.exe.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        let mut m = data_dir_matcher(&[name.as_str()], instance, ctx.env);
        m.exe_path = Some(normalize_path(&install.exe, ctx.env.os));
        m.uses_data_dir = uses_data_dir(install);
        m
    }

    fn brand(&self, install: &Installation) -> Brand {
        Brand {
            color: opt_str(install, "color").unwrap_or("#8E8E93").into(),
            letter: install.name.chars().next().unwrap_or('A').to_uppercase().to_string(),
            logo: "generic".into(),
        }
    }
}
