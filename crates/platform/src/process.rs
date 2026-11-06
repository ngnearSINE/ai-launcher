use std::collections::HashSet;
use std::path::PathBuf;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// A running process as seen by the launcher.
#[derive(Debug, Clone)]
pub struct Proc {
    pub pid: u32,
    pub parent: Option<u32>,
    /// Lower-case executable file name (`claude.exe`).
    pub name: String,
    pub exe: Option<PathBuf>,
    /// Arguments without argv[0].
    pub args: Vec<String>,
}

/// Keeps one `System` so repeated scans reuse sysinfo's caches.
pub struct ProcessScanner {
    sys: System,
}

impl Default for ProcessScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessScanner {
    pub fn new() -> Self {
        Self { sys: System::new() }
    }

    /// Processes whose executable name is in `names` (lower-case). Command lines are only read
    /// for those, which keeps a scan cheap.
    pub fn scan(&mut self, names: &HashSet<String>) -> Vec<Proc> {
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        let wanted: Vec<sysinfo::Pid> = self
            .sys
            .processes()
            .iter()
            .filter(|(_, p)| names.contains(&p.name().to_string_lossy().to_lowercase()))
            .map(|(pid, _)| *pid)
            .collect();
        if wanted.is_empty() {
            return Vec::new();
        }
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&wanted),
            false,
            ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet).with_exe(UpdateKind::OnlyIfNotSet),
        );
        wanted
            .iter()
            .filter_map(|pid| self.sys.process(*pid))
            .map(|p| Proc {
                pid: p.pid().as_u32(),
                parent: p.parent().map(|x| x.as_u32()),
                name: p.name().to_string_lossy().to_lowercase(),
                exe: p.exe().map(PathBuf::from),
                args: p.cmd().iter().skip(1).map(|a| a.to_string_lossy().to_string()).collect(),
            })
            .collect()
    }

    /// Ends processes immediately (used for "force quit"). Returns how many were signalled.
    pub fn kill(&mut self, pids: &[u32]) -> usize {
        let list: Vec<sysinfo::Pid> = pids.iter().map(|p| sysinfo::Pid::from_u32(*p)).collect();
        self.sys.refresh_processes_specifics(ProcessesToUpdate::Some(&list), false, ProcessRefreshKind::nothing());
        list.iter().filter_map(|p| self.sys.process(*p)).filter(|p| p.kill()).count()
    }

    /// All descendants of `root` (inclusive) among the last scan's processes.
    pub fn family(procs: &[Proc], root: u32) -> Vec<u32> {
        let mut out = vec![root];
        let mut i = 0;
        while i < out.len() {
            let cur = out[i];
            let kids: Vec<u32> = procs.iter().filter(|p| p.parent == Some(cur) && !out.contains(&p.pid)).map(|p| p.pid).collect();
            out.extend(kids);
            i += 1;
        }
        out
    }
}
