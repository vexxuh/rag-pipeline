use anyhow::{Context, Result};
use rig::client::completion::CompletionClientDyn;
use rig::client::embeddings::EmbeddingsClientDyn;
use rig::client::{ProviderClient, ProviderValue};
use rig::providers::{
    anthropic, cohere, deepseek, gemini, groq, mistral, ollama, openai, openrouter, perplexity,
    together, xai,
};

fn create_provider_boxed(provider: &str, api_key: &str) -> Result<Box<dyn ProviderClient>> {
    let value = ProviderValue::Simple(api_key.to_string());

    let boxed: Box<dyn ProviderClient> = match provider.to_lowercase().as_str() {
        "openai" => {
            let c: openai::Client<reqwest::Client> = openai::Client::from_val(value);
            c.boxed()
        }
        "anthropic" => {
            let c: anthropic::Client<reqwest::Client> = anthropic::Client::from_val(value);
            c.boxed()
        }
        "groq" => {
            let c: groq::Client<reqwest::Client> = groq::Client::from_val(value);
            c.boxed()
        }
        "deepseek" => {
            let c: deepseek::Client<reqwest::Client> = deepseek::Client::from_val(value);
            c.boxed()
        }
        "gemini" | "google" => {
            let c: gemini::Client<reqwest::Client> = gemini::Client::from_val(value);
            c.boxed()
        }
        "cohere" => {
            let c: cohere::Client<reqwest::Client> = cohere::Client::from_val(value);
            c.boxed()
        }
        "mistral" => {
            let c: mistral::Client<reqwest::Client> = mistral::Client::from_val(value);
            c.boxed()
        }
        "openrouter" => {
            let c: openrouter::Client<reqwest::Client> = openrouter::Client::from_val(value);
            c.boxed()
        }
        "perplexity" => {
            let c: perplexity::Client<reqwest::Client> = perplexity::Client::from_val(value);
            c.boxed()
        }
        "together" => {
            let c: together::Client<reqwest::Client> = together::Client::from_val(value);
            c.boxed()
        }
        "xai" => {
            let c: xai::Client<reqwest::Client> = xai::Client::from_val(value);
            c.boxed()
        }
        "ollama" => {
            let c: ollama::Client<reqwest::Client> = ollama::Client::from_val(value);
            c.boxed()
        }
        other => return Err(anyhow::anyhow!("Unsupported provider: {other}")),
    };

    Ok(boxed)
}

pub fn create_completion_client(
    provider: &str,
    api_key: &str,
) -> Result<Box<dyn CompletionClientDyn>> {
    let boxed = create_provider_boxed(provider, api_key)?;
    boxed
        .as_completion()
        .context(format!("Provider '{provider}' does not support completions"))
}

pub fn create_embeddings_client(
    provider: &str,
    api_key: &str,
) -> Result<Box<dyn EmbeddingsClientDyn>> {
    let boxed = create_provider_boxed(provider, api_key)?;
    boxed
        .as_embeddings()
        .context(format!("Provider '{provider}' does not support embeddings"))
}

pub fn supported_providers() -> Vec<ProviderInfo> {
    vec![
        ProviderInfo {
            id: "openai",
            name: "OpenAI",
            supports_completion: true,
            supports_embeddings: true,
            default_model: "gpt-4o",
            default_embedding_model: Some("text-embedding-3-small"),
            completion_models: &[
                ModelEntry { id: "gpt-4o", display_name: "GPT-4o" },
                ModelEntry { id: "gpt-4o-mini", display_name: "GPT-4o Mini" },
                ModelEntry { id: "gpt-4-turbo", display_name: "GPT-4 Turbo" },
                ModelEntry { id: "gpt-4", display_name: "GPT-4" },
                ModelEntry { id: "gpt-3.5-turbo", display_name: "GPT-3.5 Turbo" },
                ModelEntry { id: "o1", display_name: "o1" },
                ModelEntry { id: "o1-mini", display_name: "o1 Mini" },
                ModelEntry { id: "o1-pro", display_name: "o1 Pro" },
                ModelEntry { id: "o3-mini", display_name: "o3 Mini" },
            ],
            embedding_models: &[
                ModelEntry { id: "text-embedding-3-small", display_name: "Embedding 3 Small" },
                ModelEntry { id: "text-embedding-3-large", display_name: "Embedding 3 Large" },
                ModelEntry { id: "text-embedding-ada-002", display_name: "Embedding Ada 002" },
            ],
        },
        ProviderInfo {
            id: "anthropic",
            name: "Anthropic",
            supports_completion: true,
            supports_embeddings: false,
            default_model: "claude-sonnet-4-20250514",
            default_embedding_model: None,
            completion_models: &[
                ModelEntry { id: "claude-opus-4-20250514", display_name: "Claude Opus 4" },
                ModelEntry { id: "claude-sonnet-4-20250514", display_name: "Claude Sonnet 4" },
                ModelEntry { id: "claude-3-5-haiku-20241022", display_name: "Claude 3.5 Haiku" },
                ModelEntry { id: "claude-3-5-sonnet-20241022", display_name: "Claude 3.5 Sonnet" },
                ModelEntry { id: "claude-3-opus-20240229", display_name: "Claude 3 Opus" },
            ],
            embedding_models: &[],
        },
        ProviderInfo {
            id: "groq",
            name: "Groq",
            supports_completion: true,
            supports_embeddings: false,
            default_model: "mixtral-8x7b-32768",
            default_embedding_model: None,
            completion_models: &[
                ModelEntry { id: "mixtral-8x7b-32768", display_name: "Mixtral 8x7B" },
                ModelEntry { id: "llama-3.3-70b-versatile", display_name: "Llama 3.3 70B" },
                ModelEntry { id: "llama-3.1-8b-instant", display_name: "Llama 3.1 8B Instant" },
                ModelEntry { id: "llama-3.1-70b-versatile", display_name: "Llama 3.1 70B" },
                ModelEntry { id: "gemma2-9b-it", display_name: "Gemma 2 9B" },
                ModelEntry { id: "deepseek-r1-distill-llama-70b", display_name: "DeepSeek R1 Distill 70B" },
            ],
            embedding_models: &[],
        },
        ProviderInfo {
            id: "deepseek",
            name: "DeepSeek",
            supports_completion: true,
            supports_embeddings: false,
            default_model: "deepseek-chat",
            default_embedding_model: None,
            completion_models: &[
                ModelEntry { id: "deepseek-chat", display_name: "DeepSeek Chat (V3)" },
                ModelEntry { id: "deepseek-reasoner", display_name: "DeepSeek Reasoner (R1)" },
            ],
            embedding_models: &[],
        },
        ProviderInfo {
            id: "gemini",
            name: "Google Gemini",
            supports_completion: true,
            supports_embeddings: true,
            default_model: "gemini-2.0-flash",
            default_embedding_model: Some("text-embedding-004"),
            completion_models: &[
                ModelEntry { id: "gemini-2.5-pro-preview-06-05", display_name: "Gemini 2.5 Pro" },
                ModelEntry { id: "gemini-2.5-flash-preview-05-20", display_name: "Gemini 2.5 Flash" },
                ModelEntry { id: "gemini-2.0-flash", display_name: "Gemini 2.0 Flash" },
                ModelEntry { id: "gemini-2.0-flash-lite", display_name: "Gemini 2.0 Flash Lite" },
                ModelEntry { id: "gemini-1.5-pro", display_name: "Gemini 1.5 Pro" },
                ModelEntry { id: "gemini-1.5-flash", display_name: "Gemini 1.5 Flash" },
            ],
            embedding_models: &[
                ModelEntry { id: "text-embedding-004", display_name: "Text Embedding 004" },
                ModelEntry { id: "embedding-001", display_name: "Embedding 001" },
            ],
        },
        ProviderInfo {
            id: "cohere",
            name: "Cohere",
            supports_completion: true,
            supports_embeddings: true,
            default_model: "command-r-plus",
            default_embedding_model: Some("embed-english-v3.0"),
            completion_models: &[
                ModelEntry { id: "command-r-plus", display_name: "Command R+" },
                ModelEntry { id: "command-r", display_name: "Command R" },
                ModelEntry { id: "command-light", display_name: "Command Light" },
                ModelEntry { id: "command", display_name: "Command" },
            ],
            embedding_models: &[
                ModelEntry { id: "embed-english-v3.0", display_name: "Embed English v3.0" },
                ModelEntry { id: "embed-multilingual-v3.0", display_name: "Embed Multilingual v3.0" },
                ModelEntry { id: "embed-english-light-v3.0", display_name: "Embed English Light v3.0" },
                ModelEntry { id: "embed-multilingual-light-v3.0", display_name: "Embed Multilingual Light v3.0" },
            ],
        },
        ProviderInfo {
            id: "mistral",
            name: "Mistral",
            supports_completion: true,
            supports_embeddings: true,
            default_model: "mistral-large-latest",
            default_embedding_model: Some("mistral-embed"),
            completion_models: &[
                ModelEntry { id: "mistral-large-latest", display_name: "Mistral Large" },
                ModelEntry { id: "mistral-medium-latest", display_name: "Mistral Medium" },
                ModelEntry { id: "mistral-small-latest", display_name: "Mistral Small" },
                ModelEntry { id: "open-mistral-nemo", display_name: "Mistral Nemo" },
                ModelEntry { id: "codestral-latest", display_name: "Codestral" },
                ModelEntry { id: "pixtral-large-latest", display_name: "Pixtral Large" },
            ],
            embedding_models: &[
                ModelEntry { id: "mistral-embed", display_name: "Mistral Embed" },
            ],
        },
        ProviderInfo {
            id: "openrouter",
            name: "OpenRouter",
            supports_completion: true,
            supports_embeddings: false,
            default_model: "anthropic/claude-sonnet-4",
            default_embedding_model: None,
            completion_models: &[
                ModelEntry { id: "anthropic/claude-sonnet-4", display_name: "Claude Sonnet 4" },
                ModelEntry { id: "anthropic/claude-3.5-sonnet", display_name: "Claude 3.5 Sonnet" },
                ModelEntry { id: "openai/gpt-4o", display_name: "GPT-4o" },
                ModelEntry { id: "openai/gpt-4o-mini", display_name: "GPT-4o Mini" },
                ModelEntry { id: "google/gemini-2.0-flash-001", display_name: "Gemini 2.0 Flash" },
                ModelEntry { id: "meta-llama/llama-3.3-70b-instruct", display_name: "Llama 3.3 70B" },
                ModelEntry { id: "deepseek/deepseek-chat", display_name: "DeepSeek Chat V3" },
                ModelEntry { id: "deepseek/deepseek-r1", display_name: "DeepSeek R1" },
                ModelEntry { id: "mistralai/mistral-large", display_name: "Mistral Large" },
                ModelEntry { id: "qwen/qwen-2.5-72b-instruct", display_name: "Qwen 2.5 72B" },
            ],
            embedding_models: &[],
        },
        ProviderInfo {
            id: "perplexity",
            name: "Perplexity",
            supports_completion: true,
            supports_embeddings: false,
            default_model: "sonar",
            default_embedding_model: None,
            completion_models: &[
                ModelEntry { id: "sonar", display_name: "Sonar" },
                ModelEntry { id: "sonar-pro", display_name: "Sonar Pro" },
                ModelEntry { id: "sonar-reasoning", display_name: "Sonar Reasoning" },
                ModelEntry { id: "sonar-reasoning-pro", display_name: "Sonar Reasoning Pro" },
            ],
            embedding_models: &[],
        },
        ProviderInfo {
            id: "together",
            name: "Together AI",
            supports_completion: true,
            supports_embeddings: true,
            default_model: "meta-llama/Llama-3.3-70B-Instruct-Turbo",
            default_embedding_model: Some("togethercomputer/m2-bert-80M-8k-retrieval"),
            completion_models: &[
                ModelEntry { id: "meta-llama/Llama-3.3-70B-Instruct-Turbo", display_name: "Llama 3.3 70B Turbo" },
                ModelEntry { id: "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo", display_name: "Llama 3.1 8B Turbo" },
                ModelEntry { id: "meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo", display_name: "Llama 3.1 70B Turbo" },
                ModelEntry { id: "meta-llama/Meta-Llama-3.1-405B-Instruct-Turbo", display_name: "Llama 3.1 405B Turbo" },
                ModelEntry { id: "Qwen/Qwen2.5-72B-Instruct-Turbo", display_name: "Qwen 2.5 72B Turbo" },
                ModelEntry { id: "mistralai/Mixtral-8x7B-Instruct-v0.1", display_name: "Mixtral 8x7B" },
                ModelEntry { id: "deepseek-ai/DeepSeek-R1", display_name: "DeepSeek R1" },
            ],
            embedding_models: &[
                ModelEntry { id: "togethercomputer/m2-bert-80M-8k-retrieval", display_name: "M2 BERT 80M 8K" },
                ModelEntry { id: "BAAI/bge-large-en-v1.5", display_name: "BGE Large EN v1.5" },
                ModelEntry { id: "BAAI/bge-base-en-v1.5", display_name: "BGE Base EN v1.5" },
            ],
        },
        ProviderInfo {
            id: "xai",
            name: "xAI",
            supports_completion: true,
            supports_embeddings: false,
            default_model: "grok-3-mini",
            default_embedding_model: None,
            completion_models: &[
                ModelEntry { id: "grok-3", display_name: "Grok 3" },
                ModelEntry { id: "grok-3-mini", display_name: "Grok 3 Mini" },
                ModelEntry { id: "grok-3-fast", display_name: "Grok 3 Fast" },
                ModelEntry { id: "grok-3-mini-fast", display_name: "Grok 3 Mini Fast" },
                ModelEntry { id: "grok-2", display_name: "Grok 2" },
            ],
            embedding_models: &[],
        },
        ProviderInfo {
            id: "ollama",
            name: "Ollama (Local)",
            supports_completion: true,
            supports_embeddings: true,
            default_model: "llama3.1:8b",
            default_embedding_model: Some("nomic-embed-text"),
            completion_models: &[
                ModelEntry { id: "llama3.1:8b", display_name: "Llama 3.1 8B" },
                ModelEntry { id: "llama3.1:70b", display_name: "Llama 3.1 70B" },
                ModelEntry { id: "llama3.2:3b", display_name: "Llama 3.2 3B" },
                ModelEntry { id: "llama3.2:1b", display_name: "Llama 3.2 1B" },
                ModelEntry { id: "mistral:7b", display_name: "Mistral 7B" },
                ModelEntry { id: "mixtral:8x7b", display_name: "Mixtral 8x7B" },
                ModelEntry { id: "gemma2:9b", display_name: "Gemma 2 9B" },
                ModelEntry { id: "gemma2:27b", display_name: "Gemma 2 27B" },
                ModelEntry { id: "phi3:mini", display_name: "Phi-3 Mini" },
                ModelEntry { id: "qwen2.5:7b", display_name: "Qwen 2.5 7B" },
                ModelEntry { id: "qwen2.5:72b", display_name: "Qwen 2.5 72B" },
                ModelEntry { id: "deepseek-r1:8b", display_name: "DeepSeek R1 8B" },
                ModelEntry { id: "deepseek-r1:70b", display_name: "DeepSeek R1 70B" },
            ],
            embedding_models: &[
                ModelEntry { id: "nomic-embed-text", display_name: "Nomic Embed Text" },
                ModelEntry { id: "mxbai-embed-large", display_name: "MxBai Embed Large" },
                ModelEntry { id: "all-minilm", display_name: "All MiniLM" },
                ModelEntry { id: "snowflake-arctic-embed", display_name: "Snowflake Arctic Embed" },
            ],
        },
    ]
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub supports_completion: bool,
    pub supports_embeddings: bool,
    pub default_model: &'static str,
    pub default_embedding_model: Option<&'static str>,
    pub completion_models: &'static [ModelEntry],
    pub embedding_models: &'static [ModelEntry],
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelEntry {
    pub id: &'static str,
    pub display_name: &'static str,
}
