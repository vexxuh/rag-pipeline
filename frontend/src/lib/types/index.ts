export interface User {
	id: string;
	username: string;
	email: string;
	role: 'admin' | 'user';
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
	role: 'user' | 'assistant';
	content: string;
	timestamp: Date;
}

export interface Document {
	id: string;
	filename: string;
	original_filename: string;
	content_type: string;
	size_bytes: number;
	status: 'uploading' | 'processing' | 'ready' | 'failed';
	error_message: string | null;
	created_at: string;
	processed_at: string | null;
}

export interface CrawlJob {
	id: string;
	url: string;
	crawl_type: 'sitemap' | 'full';
	status: 'pending' | 'running' | 'completed' | 'failed';
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
}

export interface ApiKey {
	id: string;
	provider: string;
	created_at: string;
}
