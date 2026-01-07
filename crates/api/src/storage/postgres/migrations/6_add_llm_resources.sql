-- Migration: Add LLM resources
-- This creates a new resource type for Large Language Models
-- LLMs can be used for topic naming in visualizations and RAG chat

CREATE TABLE IF NOT EXISTS LLMS (
    llm_id               SERIAL PRIMARY KEY,
    name                 TEXT                     NOT NULL,
    owner                TEXT                     NOT NULL,
    provider             TEXT                     NOT NULL,  -- 'openai' | 'cohere'
    base_url             TEXT                     NOT NULL,
    api_key              TEXT                     NULL,      -- Optional for public endpoints
    config               JSONB                    NOT NULL DEFAULT '{}',  -- model, temperature, max_tokens, etc.
    is_public            BOOLEAN                  NOT NULL DEFAULT FALSE,
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Create index on owner for efficient user queries
CREATE INDEX IF NOT EXISTS idx_llms_owner
    ON LLMS(owner);

-- Create index on provider for filtering by LLM provider
CREATE INDEX IF NOT EXISTS idx_llms_provider
    ON LLMS(provider);

-- Create partial index for public LLMs
CREATE INDEX IF NOT EXISTS idx_llms_public
    ON LLMS(is_public) WHERE is_public = TRUE;

-- Create composite index for marketplace queries
CREATE INDEX IF NOT EXISTS idx_llms_public_owner
    ON LLMS(is_public, owner);
