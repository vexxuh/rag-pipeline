export interface User {
  id: string;
  username: string;
  email: string;
  role: "admin" | "maintainer" | "user";
  created_at: string;
  updated_at: string;
}

export interface AuthState {
  user: User | null;
  token: string | null;
}

export interface ApiError {
  error: string;
  status: number;
}

export interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  timestamp: Date;
}

export interface Document {
  id: string;
  filename: string;
  original_filename: string;
  content_type: string;
  size_bytes: number;
  status: "uploading" | "processing" | "ready" | "failed";
  error_message: string | null;
  created_at: string;
  processed_at: string | null;
}

export interface CrawlJob {
  id: string;
  url: string;
  crawl_type: "sitemap" | "full";
  status: "pending" | "running" | "completed" | "failed";
  pages_found: number;
  pages_processed: number;
  error_message: string | null;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
}

export interface LlmPreferences {
  preferred_provider: string;
  preferred_model: string;
  preferred_embedding_model: string;
  system_prompt: string;
}

export interface ApiKey {
  id: string;
  provider: string;
  created_at: string;
}

export interface Conversation {
  id: string;
  user_id: string;
  title: string;
  created_at: string;
  updated_at: string;
}

export interface Message {
  id: string;
  conversation_id: string;
  role: "user" | "assistant";
  content: string;
  created_at: string;
}

export interface ConversationWithMessages extends Conversation {
  messages: Message[];
}

export interface AdminProvider {
  id: string;
  provider_id: string;
  display_name: string;
  enabled: boolean;
  supports_completion: boolean;
  supports_embeddings: boolean;
  created_at: string;
}

export interface AdminModel {
  id: string;
  provider_id: string;
  model_id: string;
  display_name: string;
  model_type: "completion" | "embedding";
  is_default: boolean;
  created_at: string;
}

export interface ConversationLog {
  id: string;
  user_id: string;
  username: string;
  email: string;
  title: string;
  message_count: number;
  created_at: string;
  updated_at: string;
}

export interface LogsResponse {
  conversations: ConversationLog[];
  total: number;
  page: number;
  per_page: number;
}

export interface LogDetail {
  id: string;
  user_id: string;
  title: string;
  created_at: string;
  updated_at: string;
  messages: Message[];
}

export interface EmbedKey {
  id: string;
  name: string;
  key_prefix: string;
  allowed_domains: string[];
  system_prompt: string;
  rate_limit: number;
  widget_title: string;
  primary_color: string;
  greeting_message: string;
  provider: string;
  model: string;
  is_active: boolean;
  total_conversations: number;
  total_messages: number;
  created_at: string;
  updated_at: string;
}

export interface CreateEmbedKeyResponse {
  embed_key: EmbedKey;
  raw_key: string;
}
