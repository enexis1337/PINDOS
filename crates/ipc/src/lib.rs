#![no_std]

use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;
use bitflags::bitflags;

/// Maximum number of endpoints
const MAX_ENDPOINTS: usize = 1024;

/// Maximum message queue depth
const MAX_QUEUE_DEPTH: usize = 16;

/// Message priority levels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessagePriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Urgent = 3,
}

/// IPC message structure
#[repr(C)]
pub struct IpcMessage {
    pub sender: usize,      // Sender endpoint ID
    pub receiver: usize,    // Receiver endpoint ID
    pub priority: MessagePriority,
    pub payload_len: usize,
    pub payload: [u8; 256], // Inline payload (up to 256 bytes)
    pub timestamp: u64,
}

impl IpcMessage {
    /// Create a new message
    pub fn new(sender: usize, receiver: usize, priority: MessagePriority, data: &[u8]) -> Self {
        let mut payload = [0u8; 256];
        let len = core::cmp::min(data.len(), 256);
        payload[..len].copy_from_slice(&data[..len]);
        
        IpcMessage {
            sender,
            receiver,
            priority,
            payload_len: len,
            payload,
            timestamp: 0, // Will be set by kernel
        }
    }

    /// Get payload as slice
    pub fn payload(&self) -> &[u8] {
        &self.payload[..self.payload_len]
    }
}

/// Endpoint types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EndpointType {
    Port,      // One-way send port
    Channel,   // Bidirectional channel
    Server,    // Server endpoint (can receive)
    Client,    // Client endpoint (can send)
}

/// Endpoint flags
bitflags! {
    pub struct EndpointFlags: u32 {
        const NONE = 0;
        const BLOCKING = 1 << 0;
        const NON_BLOCKING = 1 << 1;
        const URGENT = 1 << 2;
        const BROADCAST = 1 << 3;
    }
}

/// IPC Endpoint - capability-based communication channel
pub struct Endpoint {
    pub id: usize,
    pub process_id: usize,
    pub endpoint_type: EndpointType,
    pub flags: EndpointFlags,
    pub peer: Option<usize>,  // Connected endpoint (for channels)
    pub message_queue: Mutex<Vec<IpcMessage>>,
    pub send_waiters: Mutex<Vec<usize>>, // Thread IDs waiting to send
    pub recv_waiters: Mutex<Vec<usize>>, // Thread IDs waiting to receive
}

impl Endpoint {
    /// Create a new endpoint
    pub fn new(id: usize, process_id: usize, endpoint_type: EndpointType, flags: EndpointFlags) -> Self {
        Endpoint {
            id,
            process_id,
            endpoint_type,
            flags,
            peer: None,
            message_queue: Mutex::new(Vec::with_capacity(MAX_QUEUE_DEPTH)),
            send_waiters: Mutex::new(Vec::new()),
            recv_waiters: Mutex::new(Vec::new()),
        }
    }

    /// Check if endpoint can send
    pub fn can_send(&self) -> bool {
        match self.endpoint_type {
            EndpointType::Port | EndpointType::Client | EndpointType::Server => true,
            EndpointType::Channel => self.peer.is_some(),
        }
    }

    /// Check if endpoint can receive
    pub fn can_receive(&self) -> bool {
        match self.endpoint_type {
            EndpointType::Server | EndpointType::Channel => true,
            EndpointType::Port | EndpointType::Client => false,
        }
    }

    /// Queue a message
    pub fn queue_message(&self, message: IpcMessage) -> bool {
        let mut queue = self.message_queue.lock();
        if queue.len() < MAX_QUEUE_DEPTH {
            queue.push(message);
            true
        } else {
            false
        }
    }

    /// Dequeue a message
    pub fn dequeue_message(&self) -> Option<IpcMessage> {
        let mut queue = self.message_queue.lock();
        queue.pop()
    }

    /// Peek at next message without removing
    pub fn peek_message(&self) -> Option<&IpcMessage> {
        let queue = self.message_queue.lock();
        queue.last()
    }
}

/// Global endpoint table
static ENDPOINT_TABLE: Mutex<[Option<Endpoint>; MAX_ENDPOINTS]> = 
    Mutex::new([None; MAX_ENDPOINTS]);

/// Next available endpoint ID
static NEXT_ENDPOINT_ID: AtomicUsize = AtomicUsize::new(1);

/// Initialize IPC subsystem
pub fn init() {
    // Create kernel IPC endpoints
    create_kernel_endpoint(0, EndpointType::Server, EndpointFlags::BLOCKING);
    create_kernel_endpoint(1, EndpointType::Client, EndpointFlags::NONE);
}

/// Create a kernel endpoint
fn create_kernel_endpoint(id: usize, endpoint_type: EndpointType, flags: EndpointFlags) {
    let mut table = ENDPOINT_TABLE.lock();
    table[id].replace(Endpoint::new(id, 0, endpoint_type, flags));
}

/// Allocate a new endpoint ID
pub fn allocate_endpoint_id() -> usize {
    let mut id = NEXT_ENDPOINT_ID.fetch_add(1, Ordering::SeqCst);
    if id >= MAX_ENDPOINTS {
        id = 1;
        NEXT_ENDPOINT_ID.store(id, Ordering::SeqCst);
    }
    id
}

/// Create a new endpoint for a process
pub fn create_endpoint(process_id: usize, endpoint_type: EndpointType, flags: EndpointFlags) -> usize {
    let id = allocate_endpoint_id();
    let mut table = ENDPOINT_TABLE.lock();
    table[id].replace(Endpoint::new(id, process_id, endpoint_type, flags));
    id
}

/// Connect two endpoints (for channels)
pub fn connect(end_a: usize, end_b: usize) -> bool {
    let mut table = ENDPOINT_TABLE.lock();
    
    if let (Some(Some(ref mut ep_a)), Some(Some(ref mut ep_b))) = 
        (table.get_mut(end_a), table.get_mut(end_b)) 
    {
        ep_a.peer = Some(end_b);
        ep_b.peer = Some(end_a);
        true
    } else {
        false
    }
}

/// Send a message via IPC
pub fn sys_send(to: usize, data: &[u8], priority: MessagePriority) -> Result<usize, IpcError> {
    let table = ENDPOINT_TABLE.lock();
    
    let endpoint = match table.get(to) {
        Some(Some(ep)) => ep,
        _ => return Err(IpcError::InvalidEndpoint),
    };
    
    if !endpoint.can_send() {
        return Err(IpcError::CannotSend);
    }
    
    // Get sender endpoint (current process's default endpoint)
    let sender_id = get_current_endpoint();
    
    // Create message
    let message = IpcMessage::new(sender_id, to, priority, data);
    
    // Queue message or block
    if endpoint.flags.contains(EndpointFlags::NON_BLOCKING) {
        if endpoint.queue_message(message) {
            Ok(data.len())
        } else {
            Err(IpcError::QueueFull)
        }
    } else {
        // Blocking send - queue and wake receiver
        endpoint.queue_message(message);
        Ok(data.len())
    }
}

/// Receive a message from an endpoint
pub fn sys_recv(from: usize, buffer: &mut [u8], blocking: bool) -> Result<usize, IpcError> {
    let table = ENDPOINT_TABLE.lock();
    
    let endpoint = match table.get(from) {
        Some(Some(ep)) => ep,
        _ => return Err(IpcError::InvalidEndpoint),
    };
    
    if !endpoint.can_receive() {
        return Err(IpcError::CannotReceive);
    }
    
    // Try to dequeue a message
    if let Some(message) = endpoint.dequeue_message() {
        let len = core::cmp::min(buffer.len(), message.payload_len);
        buffer[..len].copy_from_slice(&message.payload[..len]);
        return Ok(len);
    }
    
    // No message available
    if !blocking {
        return Err(IpcError::NoMessage);
    }
    
    // Would block - in real implementation, block current thread
    Err(IpcError::WouldBlock)
}

/// Call (synchronous request-response IPC)
pub fn sys_call(to: usize, request: &[u8], response: &mut [u8]) -> Result<usize, IpcError> {
    // Send request
    let send_result = sys_send(to, request, MessagePriority::Normal);
    if send_result.is_err() {
        return send_result;
    }
    
    // Wait for response (from the peer's response endpoint)
    let response_endpoint = get_response_endpoint(to);
    sys_recv(response_endpoint, response, true)
}

/// Get the current process's default endpoint
fn get_current_endpoint() -> usize {
    // In real implementation, this would be stored in the TCB
    0
}

/// Get response endpoint for a server
fn get_response_endpoint(server_id: usize) -> usize {
    server_id + 1 // Response endpoint is server_id + 1
}

/// IPC error types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IpcError {
    InvalidEndpoint,
    CannotSend,
    CannotReceive,
    QueueFull,
    NoMessage,
    WouldBlock,
    InvalidData,
}

/// Register an endpoint with a name (for service discovery)
pub fn register_name(name: &str, endpoint_id: usize) {
    // In real implementation, this would use a kernel name service
    let _ = name;
    let _ = endpoint_id;
}

/// Look up an endpoint by name
pub fn lookup_name(name: &str) -> Option<usize> {
    // In real implementation, this would query the kernel name service
    match name {
        "init" => Some(0),
        "vfs" => Some(10),
        "network" => Some(20),
        _ => None,
    }
}