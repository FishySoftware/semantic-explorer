"""
LLM Abstraction Layer

Supports multiple LLM providers (Cohere, OpenAI) for topic naming.
Requires API key to be passed via LLMConfig from the job message.
"""

import json
import logging
import time
from typing import List, Optional, cast

import cohere
from cohere.types import UserChatMessageV2, TextAssistantMessageResponseContentItem
from openai import OpenAI

try:
    # Try relative imports (for package execution)
    from .models import LLMConfig
except ImportError:
    # Fallback to absolute imports (for direct script execution)
    from models import LLMConfig

logger = logging.getLogger(__name__)


class LLMProvider:
    """Flexible LLM provider interface with API key from config."""

    def __init__(self):
        """Initialize LLM provider (clients created on-demand per request)."""
        self.request_count = 0
        logger.info("LLM Provider initialized (clients will be created on-demand)")
        logger.debug("LLM Provider ready for topic naming requests")

    async def generate_topic_name(
        self, texts: List[str], llm_config: LLMConfig, max_tokens: int = 50
    ) -> str:
        """
        Generate a topic name for cluster texts using the specified LLM.

        Matches SAMPLE pattern: uses Cohere API with JSON-formatted requests.

        Args:
            texts: Sample texts from the cluster (representative documents)
            llm_config: LLM configuration with provider, model, and API key
            max_tokens: Maximum response tokens

        Returns:
            Generated topic name (2-4 words)

        Raises:
            Exception: If LLM call fails
        """
        request_start = time.time()
        self.request_count += 1

        logger.debug(
            f"LLM request #{self.request_count}: {llm_config.provider}/{llm_config.model} "
            f"(texts: {len(texts)}, max_tokens: {max_tokens})"
        )

        if not llm_config or not llm_config.api_key:
            logger.error("LLM config or API key is missing")
            raise ValueError("LLM config or API key is missing")

        try:
            if llm_config.provider.lower() == "cohere":
                result = await self._generate_cohere(texts, llm_config, max_tokens)
            elif llm_config.provider.lower() == "openai":
                result = await self._generate_openai(texts, llm_config, max_tokens)
            else:
                logger.error(f"Unknown LLM provider: {llm_config.provider}")
                raise ValueError(f"Unknown LLM provider: {llm_config.provider}")

            elapsed = time.time() - request_start
            logger.info(
                f"LLM request #{self.request_count} completed in {elapsed:.3f}s: '{result}'"
            )
            return result
        except Exception as e:
            elapsed = time.time() - request_start
            logger.error(
                f"LLM request #{self.request_count} failed in {elapsed:.3f}s: {type(e).__name__}: {e}",
                exc_info=True,
            )
            raise

    async def _generate_cohere(
        self, texts: List[str], llm_config: LLMConfig, max_tokens: int
    ) -> str:
        """
        Generate topic name using Cohere API.

        Follows SAMPLE pattern: sends sample texts, gets JSON response with topic name.
        """
        cohere_start = time.time()
        try:
            logger.debug(f"Initializing Cohere client for model {llm_config.model}")
            client = cohere.ClientV2(api_key=llm_config.api_key)
            logger.debug("Cohere client initialized")

            # Prepare sample texts for analysis (up to 5 representative texts)
            sample_texts = texts[:5]
            samples_text = "\n".join(sample_texts)

            # Build prompt matching SAMPLE style
            prompt = (
                f"These are representative texts from a document cluster:\n\n"
                f"{samples_text}\n\n"
                f"Provide a short, concise topic name (2-4 words) that captures the main theme. "
                f"Respond with ONLY the topic name, nothing else."
            )

            api_call_start = time.time()
            logger.info(
                f"Calling Cohere {llm_config.model} API (prompt length: {len(prompt)})"
            )

            # Call Cohere Chat API (v2 uses messages list format with proper types)
            response = client.chat(
                model=llm_config.model or "command-r-plus",
                messages=[UserChatMessageV2(role="user", content=prompt)],
                max_tokens=max_tokens,
                temperature=0.3,  # Lower temperature for consistency
            )

            api_elapsed = time.time() - api_call_start
            logger.info(f"Cohere API call completed in {api_elapsed:.3f}s")

            # Extract text from response message
            if not response.message.content:
                logger.error("Cohere returned empty response content")
                raise ValueError("Cohere returned empty response content")

            # Get the text content from the first content item
            content_item = response.message.content[0]
            # Cast to TextAssistantMessageResponseContentItem to access text attribute
            text_item = cast(TextAssistantMessageResponseContentItem, content_item)
            topic_name = text_item.text.strip()

            cohere_elapsed = time.time() - cohere_start
            logger.info(
                f"Cohere {llm_config.model} generated topic '{topic_name}' "
                f"in {cohere_elapsed:.3f}s (API: {api_elapsed:.3f}s)"
            )
            return topic_name

        except Exception as e:
            cohere_elapsed = time.time() - cohere_start
            logger.error(
                f"Cohere topic generation failed in {cohere_elapsed:.3f}s: "
                f"{type(e).__name__}: {e}",
                exc_info=True,
            )
            raise

    async def _generate_openai(
        self, texts: List[str], llm_config: LLMConfig, max_tokens: int
    ) -> str:
        """
        Generate topic name using OpenAI API.

        Follows same pattern as Cohere: sends sample texts, gets topic name.
        """
        openai_start = time.time()
        try:
            logger.debug(f"Initializing OpenAI client for model {llm_config.model}")
            client = OpenAI(api_key=llm_config.api_key)
            logger.debug("OpenAI client initialized")

            sample_texts = texts[:5]
            samples_text = "\n".join(sample_texts)

            prompt = (
                f"These are representative texts from a document cluster:\n\n"
                f"{samples_text}\n\n"
                f"Provide a short, concise topic name (2-4 words) that captures the main theme. "
                f"Respond with ONLY the topic name, nothing else."
            )

            api_call_start = time.time()
            logger.debug(
                f"Calling OpenAI {llm_config.model} API (prompt length: {len(prompt)})"
            )

            # Call OpenAI API
            response = client.chat.completions.create(
                model=llm_config.model or "gpt-4",
                messages=[{"role": "user", "content": prompt}],
                max_tokens=max_tokens,
                temperature=0.3,  # Lower temperature for consistency
            )

            api_elapsed = time.time() - api_call_start
            logger.debug(f"OpenAI API call completed in {api_elapsed:.3f}s")

            message_content = response.choices[0].message.content
            if message_content is None:
                logger.error("OpenAI returned empty response content")
                raise ValueError("OpenAI returned empty response content")
            topic_name = message_content.strip()
            openai_elapsed = time.time() - openai_start
            logger.info(
                f"OpenAI {llm_config.model} generated topic '{topic_name}' "
                f"in {openai_elapsed:.3f}s (API: {api_elapsed:.3f}s)"
            )
            return topic_name

        except Exception as e:
            openai_elapsed = time.time() - openai_start
            logger.error(
                f"OpenAI topic generation failed in {openai_elapsed:.3f}s: "
                f"{type(e).__name__}: {e}",
                exc_info=True,
            )
            raise
