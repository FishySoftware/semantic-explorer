-- Add table to store retrieved documents for chat messages
CREATE TABLE IF NOT EXISTS chat_message_retrieved_documents (
    id SERIAL PRIMARY KEY,
    message_id INTEGER NOT NULL REFERENCES chat_messages(message_id) ON DELETE CASCADE,
    document_id TEXT,
    text TEXT NOT NULL,
    similarity_score REAL NOT NULL,
    source TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_chat_message_retrieved_documents_message_id 
    ON chat_message_retrieved_documents(message_id);
CREATE INDEX IF NOT EXISTS idx_chat_message_retrieved_documents_score 
    ON chat_message_retrieved_documents(message_id, similarity_score DESC);
