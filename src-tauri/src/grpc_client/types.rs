use serde_json::Value;
use std::error::Error;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};

// 公共错误类型
pub type GrpcResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

// 流式响应回调类型
pub type StreamCallback = Arc<dyn Fn(Value) -> Result<(), Box<dyn Error + Send + Sync>> + Send + Sync>;

// 缓存配置
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_entries: usize,
    pub ttl: Duration,
    pub enable_compression: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            ttl: Duration::from_secs(300), // 5分钟
            enable_compression: false,
        }
    }
}

// 缓存项
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub value: Value,
    pub created_at: Instant,
    pub access_count: u64,
    pub last_accessed: Instant,
}

impl CacheEntry {
    pub fn new(value: Value) -> Self {
        let now = Instant::now();
        Self {
            value,
            created_at: now,
            access_count: 0,
            last_accessed: now,
        }
    }
    
    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }
    
    pub fn access(&mut self) -> &Value {
        self.access_count += 1;
        self.last_accessed = Instant::now();
        &self.value
    }
}

// 简单的 LRU 缓存
#[derive(Debug)]
pub struct LruCache {
    entries: HashMap<String, CacheEntry>,
    config: CacheConfig,
    hits: u64,
    misses: u64,
}

impl LruCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            entries: HashMap::new(),
            config,
            hits: 0,
            misses: 0,
        }
    }
    
    pub fn get(&mut self, key: &str) -> Option<Value> {
        // 先检查是否过期，如果过期则移除
        if let Some(entry) = self.entries.get(key) {
            if entry.is_expired(self.config.ttl) {
                self.entries.remove(key);
                self.misses += 1;
                return None;
            }
        }
        
        // 如果没过期，则访问并返回克隆
        if let Some(entry) = self.entries.get_mut(key) {
            self.hits += 1;
            let value = entry.access().clone();
            Some(value)
        } else {
            self.misses += 1;
            None
        }
    }
    
    pub fn put(&mut self, key: String, value: Value) {
        // 如果超过最大数量，移除最旧的条目
        if self.entries.len() >= self.config.max_entries {
            self.evict_oldest();
        }
        
        self.entries.insert(key, CacheEntry::new(value));
    }
    
    fn evict_oldest(&mut self) {
        if let Some((oldest_key, _)) = self.entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed) {
            let oldest_key = oldest_key.clone();
            self.entries.remove(&oldest_key);
        }
    }
    
    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }
    
    pub fn get_stats(&self) -> serde_json::Value {
        let total_requests = self.hits + self.misses;
        let hit_rate = if total_requests > 0 {
            self.hits as f64 / total_requests as f64
        } else {
            0.0
        };
        
        serde_json::json!({
            "entries": self.entries.len(),
            "max_entries": self.config.max_entries,
            "hits": self.hits,
            "misses": self.misses,
            "hit_rate": hit_rate,
            "ttl_seconds": self.config.ttl.as_secs()
        })
    }
    
    pub fn cleanup_expired(&mut self) -> usize {
        let mut removed = 0;
        let ttl = self.config.ttl;
        
        self.entries.retain(|_, entry| {
            if entry.is_expired(ttl) {
                removed += 1;
                false
            } else {
                true
            }
        });
        
        removed
    }
}

// 流式请求配置
#[derive(Clone)]
pub struct StreamConfig {
    pub enable_streaming: bool,
    pub callback: Option<StreamCallback>,
    pub max_messages: Option<usize>,
}

// 手动实现 Debug，因为 StreamCallback 无法自动 derive Debug
impl std::fmt::Debug for StreamConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamConfig")
            .field("enable_streaming", &self.enable_streaming)
            .field("callback", &if self.callback.is_some() { "Some(Fn)" } else { "None" })
            .field("max_messages", &self.max_messages)
            .finish()
    }
}

// 服务类型枚举
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum ServiceType {
    State,
    Ui,
    Mcp,
    File,
    Models,
    Task,
    Account,
    Browser,
    Commands,
    Checkpoints,
    Slash,
    Web,
}

impl ServiceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ServiceType::State => "cline.StateService",
            ServiceType::Ui => "cline.UiService", 
            ServiceType::Mcp => "cline.McpService",
            ServiceType::File => "cline.FileService",
            ServiceType::Models => "cline.ModelsService",
            ServiceType::Task => "cline.TaskService",
            ServiceType::Account => "cline.AccountService",
            ServiceType::Browser => "cline.BrowserService",
            ServiceType::Commands => "cline.CommandsService",
            ServiceType::Checkpoints => "cline.CheckpointsService",
            ServiceType::Slash => "cline.SlashService",
            ServiceType::Web => "cline.WebService",
        }
    }
}

// gRPC 请求响应的标准接口
// 使用枚举而不是 trait object 来避免 dyn compatibility 问题
#[derive(Debug)]
pub enum ServiceHandler {
    State(crate::grpc_client::services::StateServiceHandler),
    Ui(crate::grpc_client::services::UiServiceHandler),
    Mcp(crate::grpc_client::services::McpServiceHandler),
}

impl ServiceHandler {
    pub async fn handle_request(&mut self, method: &str, message: &Value) -> GrpcResult<Value> {
        self.handle_request_with_config(method, message, None).await
    }
    
    pub async fn handle_request_with_config(&mut self, method: &str, message: &Value, stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        match self {
            ServiceHandler::State(handler) => handler.handle_request_with_config(method, message, stream_config).await,
            ServiceHandler::Ui(handler) => handler.handle_request_with_config(method, message, stream_config).await,
            ServiceHandler::Mcp(handler) => handler.handle_request_with_config(method, message, stream_config).await,
        }
    }
}