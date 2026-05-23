#![no_std]

use alloc::string::String;

/// Service state machine implementation
pub struct ServiceStateMachine {
    current_state: super::ServiceState,
    target_state: super::ServiceState,
    failure_count: u32,
    restart_attempts: u32,
    last_start_time: u64,
    last_stop_time: u64,
}

impl ServiceStateMachine {
    /// Create a new state machine
    pub fn new() -> Self {
        ServiceStateMachine {
            current_state: super::ServiceState::Dead,
            target_state: super::ServiceState::Dead,
            failure_count: 0,
            restart_attempts: 0,
            last_start_time: 0,
            last_stop_time: 0,
        }
    }

    /// Get current state
    pub fn current_state(&self) -> super::ServiceState {
        self.current_state
    }

    /// Get target state
    pub fn target_state(&self) -> super::ServiceState {
        self.target_state
    }

    /// Set target state
    pub fn set_target_state(&mut self, state: super::ServiceState) {
        self.target_state = state;
    }

    /// Start the service
    pub fn start(&mut self) -> Result<(), StateError> {
        match self.current_state {
            super::ServiceState::Running => Ok(()),
            super::ServiceState::Starting => Ok(()), // Already starting
            super::ServiceState::Stopping => Err(StateError::CurrentlyStopping),
            super::ServiceState::Failed => {
                // Reset failure count on new start attempt
                self.failure_count = 0;
                self.transition_to(super::ServiceState::Starting);
                Ok(())
            }
            super::ServiceState::Dead | super::ServiceState::Unknown => {
                self.transition_to(super::ServiceState::Starting);
                Ok(())
            }
            super::ServiceState::Rebooting => Err(StateError::InvalidTransition),
        }
    }

    /// Stop the service
    pub fn stop(&mut self) -> Result<(), StateError> {
        match self.current_state {
            super::ServiceState::Dead => Ok(()),
            super::ServiceState::Stopping => Ok(()),
            super::ServiceState::Running | super::ServiceState::Starting => {
                self.transition_to(super::ServiceState::Stopping);
                Ok(())
            }
            super::ServiceState::Failed => {
                self.transition_to(super::ServiceState::Stopping);
                Ok(())
            }
            super::ServiceState::Unknown => {
                self.transition_to(super::ServiceState::Stopping);
                Ok(())
            }
            super::ServiceState::Rebooting => Err(StateError::InvalidTransition),
        }
    }

    /// Mark service as successfully started
    pub fn mark_started(&mut self) {
        if self.current_state == super::ServiceState::Starting {
            self.transition_to(super::ServiceState::Running);
            self.failure_count = 0;
            self.restart_attempts = 0;
        }
    }

    /// Mark service as successfully stopped
    pub fn mark_stopped(&mut self) {
        if self.current_state == super::ServiceState::Stopping {
            self.transition_to(super::ServiceState::Dead);
        }
    }

    /// Mark service as failed
    pub fn mark_failed(&mut self) {
        if self.current_state == super::ServiceState::Starting 
           || self.current_state == super::ServiceState::Running 
           || self.current_state == super::ServiceState::Stopping 
           || self.current_state == super::ServiceState::Starting 
        {
            self.transition_to(super::ServiceState::Failed);
            self.failure_count += 1;
        }
    }

    /// Check if service should be restarted
    pub fn should_restart(&self, policy: super::RestartPolicy) -> bool {
        match policy {
            super::RestartPolicy::No => false,
            super::RestartPolicy::OnSuccess => self.current_state == super::ServiceState::Running,
            super::RestartPolicy::OnFailure => self.current_state == super::ServiceState::Failed,
            super::RestartPolicy::OnAbnormal => {
                self.current_state == super::ServiceState::Failed 
                || self.current_state == super::ServiceState::Unknown
            }
            super::RestartPolicy::OnWatchdog => false, // Would need watchdog timer
            super::RestartPolicy::Always => true,
            super::RestartPolicy::OnSuccessOrAbnormal => {
                self.current_state == super::ServiceState::Running 
                || self.current_state == super::ServiceState::Failed
            }
            super::RestartPolicy::OnSuccessOrFailure => {
                self.current_state == super::ServiceState::Running 
                || self.current_state == super::ServiceState::Failed
            }
            super::RestartPolicy::Unknown => false,
        }
    }

    /// Get failure count
    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    /// Reset failure count
    pub fn reset_failure_count(&mut self) {
        self.failure_count = 0;
    }

    /// Transition to a new state
    fn transition_to(&mut self, new_state: super::ServiceState) {
        // Validate transition
        if !self.is_valid_transition(self.current_state, new_state) {
            return;
        }
        
        self.current_state = new_state;
    }

    /// Check if a state transition is valid
    fn is_valid_transition(&self, from: super::ServiceState, to: super::ServiceState) -> bool {
        match (from, to) {
            // Valid transitions
            (super::ServiceState::Dead, super::ServiceState::Starting) => true,
            (super::ServiceState::Dead, super::ServiceState::Unknown) => true,
            (super::ServiceState::Starting, super::ServiceState::Running) => true,
            (super::ServiceState::Starting, super::ServiceState::Failed) => true,
            (super::ServiceState::Starting, super::ServiceState::Dead) => true,
            (super::ServiceState::Running, super::ServiceState::Stopping) => true,
            (super::ServiceState::Running, super::ServiceState::Failed) => true,
            (super::ServiceState::Running, super::ServiceState::Dead) => true,
            (super::ServiceState::Stopping, super::ServiceState::Dead) => true,
            (super::ServiceState::Stopping, super::ServiceState::Failed) => true,
            (super::ServiceState::Failed, super::ServiceState::Starting) => true,
            (super::ServiceState::Failed, super::ServiceState::Dead) => true,
            (super::ServiceState::Unknown, super::ServiceState::Starting) => true,
            (super::ServiceState::Unknown, super::ServiceState::Dead) => true,
            
            // Self-transitions are valid
            (s, s) => true,
            
            // Invalid transitions
            _ => false,
        }
    }
}

/// State machine errors
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StateError {
    CurrentlyStopping,
    InvalidTransition,
    NotStarted,
    AlreadyStarted,
    Timeout,
}