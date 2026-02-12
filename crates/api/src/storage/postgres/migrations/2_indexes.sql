-- collections
CREATE INDEX idx_collections_owner_created ON collections(owner_id, created_at DESC);
CREATE INDEX idx_collections_public_created ON collections(created_at DESC) WHERE is_public = TRUE;
CREATE INDEX idx_collections_title_tsvector ON collections USING GIN (to_tsvector('english', title));
CREATE INDEX idx_collections_tags ON collections USING GIN (tags);
CREATE INDEX idx_collections_owner_title ON collections(owner_id, title);

-- datasets
CREATE INDEX idx_datasets_owner_created ON datasets(owner_id, created_at DESC);
CREATE INDEX idx_datasets_public_created ON datasets(created_at DESC) WHERE is_public = TRUE;
CREATE INDEX idx_datasets_title_tsvector ON datasets USING GIN (to_tsvector('english', title));
CREATE INDEX idx_datasets_tags ON datasets USING GIN (tags);
CREATE INDEX idx_datasets_owner_title ON datasets(owner_id, title);

-- dataset_items
CREATE INDEX idx_dataset_items_dataset_created ON dataset_items(dataset_id, created_at DESC);
CREATE UNIQUE INDEX idx_dataset_items_dataset_title_unique ON dataset_items(dataset_id, title);
CREATE INDEX idx_dataset_items_dataset_item_desc ON dataset_items(dataset_id, item_id DESC);
CREATE INDEX idx_dataset_items_watermark ON dataset_items(dataset_id, COALESCE(updated_at, created_at) ASC, item_id ASC);

-- embedders
CREATE INDEX idx_embedders_owner_created ON embedders(owner_id, created_at DESC);
CREATE INDEX idx_embedders_owner_public ON embedders(owner_id, is_public);
CREATE INDEX idx_embedders_provider ON embedders(provider);
CREATE INDEX idx_embedders_public_created ON embedders(created_at DESC) WHERE is_public = TRUE;

-- llms
CREATE INDEX idx_llms_owner_created ON llms(owner_id, created_at DESC);
CREATE INDEX idx_llms_owner_public ON llms(owner_id, is_public);
CREATE INDEX idx_llms_provider ON llms(provider);
CREATE INDEX idx_llms_public_created ON llms(created_at DESC) WHERE is_public = TRUE;

-- collection_transforms
CREATE INDEX idx_collection_transforms_owner_created ON collection_transforms(owner_id, created_at DESC);
CREATE INDEX idx_collection_transforms_collection ON collection_transforms(collection_id, is_enabled);
CREATE INDEX idx_collection_transforms_dataset ON collection_transforms(dataset_id);
CREATE INDEX idx_collection_transforms_enabled ON collection_transforms(owner_id, is_enabled) WHERE is_enabled = TRUE;
CREATE INDEX idx_collection_transforms_enabled_created ON collection_transforms(is_enabled, created_at DESC) WHERE is_enabled = TRUE;

-- dataset_transforms
CREATE INDEX idx_dataset_transforms_owner_created ON dataset_transforms(owner_id, created_at DESC);
CREATE INDEX idx_dataset_transforms_source_enabled ON dataset_transforms(source_dataset_id, is_enabled);
CREATE INDEX idx_dataset_transforms_source_owner ON dataset_transforms(source_dataset_id, owner_id);
CREATE INDEX idx_dataset_transforms_enabled ON dataset_transforms(owner_id, is_enabled) WHERE is_enabled = TRUE;
CREATE INDEX idx_dataset_transforms_enabled_created ON dataset_transforms(is_enabled, created_at DESC) WHERE is_enabled = TRUE;

-- embedded_datasets
CREATE INDEX idx_embedded_datasets_owner_created ON embedded_datasets(owner_id, created_at DESC);
CREATE INDEX idx_embedded_datasets_transform ON embedded_datasets(dataset_transform_id);
CREATE INDEX idx_embedded_datasets_collection ON embedded_datasets(collection_name);
CREATE INDEX idx_embedded_datasets_last_processed ON embedded_datasets(embedded_dataset_id, last_processed_at);
CREATE INDEX idx_embedded_datasets_standalone ON embedded_datasets(dataset_transform_id) WHERE dataset_transform_id = 0;
CREATE INDEX idx_embedded_datasets_non_standalone ON embedded_datasets(dataset_transform_id) WHERE dataset_transform_id != 0;
CREATE INDEX idx_embedded_datasets_source_owner ON embedded_datasets(source_dataset_id, owner_id);
CREATE INDEX idx_embedded_datasets_scan_lock ON embedded_datasets(embedded_dataset_id, scan_locked_at) WHERE scan_locked_at IS NOT NULL;

-- dataset_transform_batches
CREATE INDEX idx_dataset_transform_batches_composite ON dataset_transform_batches(dataset_transform_id, status, processed_at DESC);
CREATE INDEX idx_dataset_transform_batches_active ON dataset_transform_batches(status) WHERE status IN ('pending', 'processing');
CREATE INDEX idx_dataset_transform_batches_failed ON dataset_transform_batches(dataset_transform_id, status) WHERE status = 'failed';

-- dataset_transform_stats
CREATE INDEX idx_dataset_transform_stats_updated ON dataset_transform_stats(updated_at DESC);

-- pending_batches
CREATE INDEX idx_pending_batches_status_retry ON pending_batches(status, next_retry_at) WHERE status = 'pending';
CREATE INDEX idx_pending_batches_transform ON pending_batches(dataset_transform_id) WHERE dataset_transform_id IS NOT NULL;
CREATE INDEX idx_pending_batches_collection_transform ON pending_batches(collection_transform_id) WHERE collection_transform_id IS NOT NULL;
CREATE INDEX idx_pending_batches_created_status ON pending_batches(created_at, status) WHERE status = 'pending';
CREATE UNIQUE INDEX idx_pending_batches_unique_batch ON pending_batches(batch_type, COALESCE(dataset_transform_id, 0), COALESCE(collection_transform_id, 0), batch_key) WHERE status = 'pending';

-- reconciliation_runs
CREATE INDEX idx_reconciliation_runs_status ON reconciliation_runs(status, started_at DESC);
CREATE INDEX idx_reconciliation_runs_transform ON reconciliation_runs(dataset_transform_id) WHERE dataset_transform_id IS NOT NULL;

-- visualization_transforms
CREATE INDEX idx_visualization_transforms_owner_created ON visualization_transforms(owner_id, created_at DESC);
CREATE INDEX idx_visualization_transforms_embedded_enabled ON visualization_transforms(embedded_dataset_id, is_enabled);
CREATE INDEX idx_visualization_transforms_embedded_owner ON visualization_transforms(embedded_dataset_id, owner_id);
CREATE INDEX idx_visualization_transforms_enabled ON visualization_transforms(owner_id, is_enabled) WHERE is_enabled = TRUE;
CREATE INDEX idx_visualization_transforms_status ON visualization_transforms(last_run_status) WHERE last_run_status IS NOT NULL;

-- visualizations
CREATE INDEX idx_visualizations_transform_created ON visualizations(visualization_transform_id, created_at DESC);
CREATE INDEX idx_visualizations_active ON visualizations(status) WHERE status IN ('pending', 'processing');

-- transform_processed_files
CREATE INDEX idx_transform_processed_files_type_id_status ON transform_processed_files(transform_type, transform_id, process_status);
CREATE INDEX idx_transform_processed_files_processed_at ON transform_processed_files(processed_at DESC);

-- chat_sessions
CREATE INDEX idx_chat_sessions_owner_updated ON chat_sessions(owner_id, updated_at DESC);
CREATE INDEX idx_chat_sessions_owner_created ON chat_sessions(owner_id, created_at DESC);
CREATE INDEX idx_chat_sessions_embedded_dataset ON chat_sessions(embedded_dataset_id);

-- chat_messages
CREATE INDEX idx_chat_messages_session_created ON chat_messages(session_id, created_at ASC) INCLUDE (role, status);

-- chat_message_retrieved_documents
CREATE INDEX idx_chat_message_retrieved_docs ON chat_message_retrieved_documents(message_id, similarity_score DESC) INCLUDE (document_id, item_title, text);

-- audit_events
CREATE INDEX idx_audit_events_timestamp ON audit_events(timestamp DESC);
CREATE INDEX idx_audit_events_user_timestamp ON audit_events(user_id, timestamp DESC);
CREATE INDEX idx_audit_events_username_display_timestamp ON audit_events(username_display, timestamp DESC);
CREATE INDEX idx_audit_events_type_timestamp ON audit_events(event_type, timestamp DESC);
CREATE INDEX idx_audit_events_resource ON audit_events(resource_type, resource_id) WHERE resource_type IS NOT NULL;
