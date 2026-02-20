import { apiGet } from '$lib/utils/api';

export interface HealthResponse {
	postgres: string;
	qdrant: string;
	s3: string;
	nats: string;
}

export interface NatsStatusResponse {
	connected: boolean;
	server_id?: string;
	server_name?: string;
	server_version?: string;
	max_payload?: number;
	subscriptions?: Array<{
		subject: string;
		queue_group?: string;
	}>;
	jetstream?: {
		streams: Array<{
			name: string;
			subjects: string[];
			consumers: Array<{
				name: string;
				num_pending: number;
				num_ack_pending: number;
			}>;
			state: {
				messages: number;
				bytes: number;
				first_seq: number;
				last_seq: number;
				consumer_count: number;
			};
		}>;
	};
}

export function getHealthLive(signal?: AbortSignal): Promise<string> {
	return apiGet<string>('/health/live', signal);
}

export function getHealthReady(signal?: AbortSignal): Promise<HealthResponse> {
	return apiGet<HealthResponse>('/health/ready', signal);
}

export function getNatsStatus(signal?: AbortSignal): Promise<NatsStatusResponse> {
	return apiGet<NatsStatusResponse>('/api/status/nats', signal);
}

export function getCurrentUser(
	signal?: AbortSignal
): Promise<{ id: string; display_name: string; email?: string }> {
	return apiGet<{ id: string; display_name: string; email?: string }>('/api/users/@me', signal);
}
