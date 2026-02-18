"""Tests for embedding generation (amem_agent.memory.embeddings) and
the normalize_embedding utility from amem_agent.llm.base.
"""

from __future__ import annotations

import math
from unittest.mock import MagicMock

import pytest

from amem_agent.llm.base import normalize_embedding
from amem_agent.memory.embeddings import generate_embedding


# ---------------------------------------------------------------------------
# normalize_embedding (from llm.base)
# ---------------------------------------------------------------------------


class TestNormalizeEmbeddingTruncate:
    def test_normalize_embedding_truncate(self):
        """A vector longer than target_dim should be truncated."""
        vec = list(range(200))
        result = normalize_embedding(vec, target_dim=128)

        assert len(result) == 128
        assert result == list(range(128))


class TestNormalizeEmbeddingPad:
    def test_normalize_embedding_pad(self):
        """A vector shorter than target_dim should be zero-padded."""
        vec = [1.0, 2.0, 3.0]
        result = normalize_embedding(vec, target_dim=128)

        assert len(result) == 128
        assert result[:3] == [1.0, 2.0, 3.0]
        assert all(v == 0.0 for v in result[3:])


class TestNormalizeEmbeddingExact:
    def test_normalize_embedding_exact(self):
        """A vector exactly equal to target_dim should be returned unchanged."""
        vec = [float(i) for i in range(128)]
        result = normalize_embedding(vec, target_dim=128)

        assert len(result) == 128
        assert result == vec


# ---------------------------------------------------------------------------
# generate_embedding (from memory.embeddings)
# ---------------------------------------------------------------------------


class TestGenerateEmbeddingSuccess:
    def test_generate_embedding_success(self):
        """Should return a vector of brain_dimension length, L2-normalised."""
        raw_vec = [1.0] * 256  # longer than 128; will be truncated then L2-normalised
        llm = MagicMock()
        llm.embed.return_value = raw_vec

        result = generate_embedding(llm, "some text", brain_dimension=128)

        assert len(result) == 128
        # After L2 normalisation of a uniform 128-dim vector, each component
        # should be 1/sqrt(128).
        expected_component = 1.0 / math.sqrt(128)
        assert abs(result[0] - expected_component) < 1e-6
        # Magnitude should be ~1.0
        magnitude = math.sqrt(sum(v * v for v in result))
        assert abs(magnitude - 1.0) < 1e-6


class TestGenerateEmbeddingFailureReturnsZero:
    def test_generate_embedding_failure_returns_zero(self):
        """When the LLM raises, generate_embedding should return a zero vector."""
        llm = MagicMock()
        llm.embed.side_effect = RuntimeError("API down")

        result = generate_embedding(llm, "some text", brain_dimension=128)

        assert len(result) == 128
        assert all(v == 0.0 for v in result)

    def test_generate_embedding_empty_content_returns_zero(self):
        """Empty content should return a zero vector without calling the LLM."""
        llm = MagicMock()

        result = generate_embedding(llm, "", brain_dimension=128)

        assert len(result) == 128
        assert all(v == 0.0 for v in result)
        llm.embed.assert_not_called()

    def test_generate_embedding_no_embed_method_returns_zero(self):
        """An LLM that lacks embed() should return a zero vector."""
        llm = object()  # no embed method

        result = generate_embedding(llm, "some text", brain_dimension=64)

        assert len(result) == 64
        assert all(v == 0.0 for v in result)

    def test_generate_embedding_short_vector_padded(self):
        """A raw vector shorter than brain_dimension should be padded then normalised."""
        raw_vec = [3.0, 4.0]  # 2 elements, brain wants 128
        llm = MagicMock()
        llm.embed.return_value = raw_vec

        result = generate_embedding(llm, "text", brain_dimension=128)

        assert len(result) == 128
        # The non-zero portion should be [3/5, 4/5] (L2-norm of [3,4] = 5)
        assert abs(result[0] - 3.0 / 5.0) < 1e-6
        assert abs(result[1] - 4.0 / 5.0) < 1e-6
        # Padded portion should be 0.0
        assert all(abs(v) < 1e-9 for v in result[2:])
