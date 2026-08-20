#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServiceState {
    Stopped,
    Starting,
    Running,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RestartPolicy {
    No,
    OnFailure,
    Always,
}

pub struct ServiceUnit {
    pub name:       &'static str,
    pub exec_path:  &'static str,
    pub state:      ServiceState,
    pub restart:    RestartPolicy,
    pub pid:        Option<u32>,
}

impl ServiceUnit {
    pub const fn new(name: &'static str, exec_path: &'static str) -> Self {
        Self {
            name,
            exec_path,
            state:   ServiceState::Stopped,
            restart: RestartPolicy::OnFailure,
            pid:     None,
        }
    }
}