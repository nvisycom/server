//! File chunks repository for managing text segments and embeddings.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use pgvector::Vector;
use uuid::Uuid;

use crate::model::{FileChunk, NewFileChunk, ScoredFileChunk, UpdateFileChunk};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for file chunk database operations.
///
/// Handles chunk lifecycle management including creation, embedding updates,
/// and semantic similarity search via pgvector.
pub trait FileChunkRepository {
    /// Creates multiple file chunks in a single transaction.
    fn create_file_chunks(
        &mut self,
        new_chunks: Vec<NewFileChunk>,
    ) -> impl Future<Output = PgResult<Vec<FileChunk>>> + Send;

    /// Updates a chunk with new data.
    fn update_file_chunk(
        &mut self,
        chunk_id: Uuid,
        updates: UpdateFileChunk,
    ) -> impl Future<Output = PgResult<FileChunk>> + Send;

    /// Deletes all chunks for a file.
    fn delete_file_chunks(&mut self, file_id: Uuid)
    -> impl Future<Output = PgResult<usize>> + Send;

    /// Lists all chunks for a specific file ordered by chunk index.
    fn list_file_chunks(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<FileChunk>>> + Send;

    /// Searches for similar chunks using cosine similarity.
    ///
    /// Returns chunks ordered by similarity (most similar first).
    fn search_similar_chunks(
        &mut self,
        query_embedding: Vector,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<FileChunk>>> + Send;

    /// Searches for similar chunks within specific files.
    fn search_similar_chunks_in_files(
        &mut self,
        query_embedding: Vector,
        file_ids: &[Uuid],
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<FileChunk>>> + Send;

    /// Searches for similar chunks within a workspace.
    fn search_similar_chunks_in_workspace(
        &mut self,
        query_embedding: Vector,
        workspace_id: Uuid,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<FileChunk>>> + Send;

    /// Searches for similar chunks within specific files with score filtering.
    ///
    /// Returns chunks with similarity score >= min_score, ordered by similarity.
    fn search_scored_chunks_in_files(
        &mut self,
        query_embedding: Vector,
        file_ids: &[Uuid],
        min_score: f64,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<ScoredFileChunk>>> + Send;

    /// Searches for similar chunks within a workspace with score filtering.
    ///
    /// Returns chunks with similarity score >= min_score, ordered by similarity.
    fn search_scored_chunks_in_workspace(
        &mut self,
        query_embedding: Vector,
        workspace_id: Uuid,
        min_score: f64,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<ScoredFileChunk>>> + Send;

    /// Gets the total chunk count for a file.
    fn count_file_chunks(&mut self, file_id: Uuid) -> impl Future<Output = PgResult<i64>> + Send;
}

impl FileChunkRepository for PgConnection {
    async fn create_file_chunks(
        &mut self,
        new_chunks: Vec<NewFileChunk>,
    ) -> PgResult<Vec<FileChunk>> {
        use schema::file_chunks;

        if new_chunks.is_empty() {
            return Ok(vec![]);
        }

        let chunks = diesel::insert_into(file_chunks::table)
            .values(&new_chunks)
            .returning(FileChunk::as_returning())
            .get_results(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn update_file_chunk(
        &mut self,
        chunk_id: Uuid,
        updates: UpdateFileChunk,
    ) -> PgResult<FileChunk> {
        use schema::file_chunks::{self, dsl};

        let chunk = diesel::update(file_chunks::table.filter(dsl::id.eq(chunk_id)))
            .set(&updates)
            .returning(FileChunk::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunk)
    }

    async fn delete_file_chunks(&mut self, file_id: Uuid) -> PgResult<usize> {
        use schema::file_chunks::{self, dsl};

        let affected = diesel::delete(file_chunks::table.filter(dsl::file_id.eq(file_id)))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(affected)
    }

    async fn list_file_chunks(&mut self, file_id: Uuid) -> PgResult<Vec<FileChunk>> {
        use schema::file_chunks::{self, dsl};

        let chunks = file_chunks::table
            .filter(dsl::file_id.eq(file_id))
            .order(dsl::chunk_index.asc())
            .select(FileChunk::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn search_similar_chunks(
        &mut self,
        query_embedding: Vector,
        limit: i64,
    ) -> PgResult<Vec<FileChunk>> {
        use pgvector::VectorExpressionMethods;
        use schema::file_chunks::{self, dsl};

        let chunks = file_chunks::table
            .order(dsl::embedding.cosine_distance(&query_embedding))
            .limit(limit)
            .select(FileChunk::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn search_similar_chunks_in_files(
        &mut self,
        query_embedding: Vector,
        file_ids: &[Uuid],
        limit: i64,
    ) -> PgResult<Vec<FileChunk>> {
        use pgvector::VectorExpressionMethods;
        use schema::file_chunks::{self, dsl};

        if file_ids.is_empty() {
            return Ok(vec![]);
        }

        let chunks = file_chunks::table
            .filter(dsl::file_id.eq_any(file_ids))
            .order(dsl::embedding.cosine_distance(&query_embedding))
            .limit(limit)
            .select(FileChunk::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn search_similar_chunks_in_workspace(
        &mut self,
        query_embedding: Vector,
        workspace_id: Uuid,
        limit: i64,
    ) -> PgResult<Vec<FileChunk>> {
        use pgvector::VectorExpressionMethods;
        use schema::file_chunks::{self, dsl};
        use schema::files;

        // Get all file IDs for the workspace
        let file_ids: Vec<Uuid> = files::table
            .filter(files::workspace_id.eq(workspace_id))
            .filter(files::deleted_at.is_null())
            .select(files::id)
            .load(self)
            .await
            .map_err(PgError::from)?;

        if file_ids.is_empty() {
            return Ok(vec![]);
        }

        let chunks = file_chunks::table
            .filter(dsl::file_id.eq_any(file_ids))
            .order(dsl::embedding.cosine_distance(&query_embedding))
            .limit(limit)
            .select(FileChunk::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks)
    }

    async fn search_scored_chunks_in_files(
        &mut self,
        query_embedding: Vector,
        file_ids: &[Uuid],
        min_score: f64,
        limit: i64,
    ) -> PgResult<Vec<ScoredFileChunk>> {
        use pgvector::VectorExpressionMethods;
        use schema::file_chunks::{self, dsl};

        if file_ids.is_empty() {
            return Ok(vec![]);
        }

        // Cosine distance ranges from 0 (identical) to 2 (opposite)
        // Score = 1 - distance, so min_score threshold means max_distance = 1 - min_score
        let max_distance = 1.0 - min_score;

        let chunks: Vec<(FileChunk, f64)> = file_chunks::table
            .filter(dsl::file_id.eq_any(file_ids))
            .filter(
                dsl::embedding
                    .cosine_distance(&query_embedding)
                    .le(max_distance),
            )
            .order(dsl::embedding.cosine_distance(&query_embedding))
            .limit(limit)
            .select((
                FileChunk::as_select(),
                (1.0.into_sql::<diesel::sql_types::Double>()
                    - dsl::embedding.cosine_distance(&query_embedding)),
            ))
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks
            .into_iter()
            .map(|(chunk, score)| ScoredFileChunk { chunk, score })
            .collect())
    }

    async fn search_scored_chunks_in_workspace(
        &mut self,
        query_embedding: Vector,
        workspace_id: Uuid,
        min_score: f64,
        limit: i64,
    ) -> PgResult<Vec<ScoredFileChunk>> {
        use pgvector::VectorExpressionMethods;
        use schema::file_chunks::{self, dsl};
        use schema::files;

        // Get all file IDs for the workspace
        let file_ids: Vec<Uuid> = files::table
            .filter(files::workspace_id.eq(workspace_id))
            .filter(files::deleted_at.is_null())
            .select(files::id)
            .load(self)
            .await
            .map_err(PgError::from)?;

        if file_ids.is_empty() {
            return Ok(vec![]);
        }

        let max_distance = 1.0 - min_score;

        let chunks: Vec<(FileChunk, f64)> = file_chunks::table
            .filter(dsl::file_id.eq_any(file_ids))
            .filter(
                dsl::embedding
                    .cosine_distance(&query_embedding)
                    .le(max_distance),
            )
            .order(dsl::embedding.cosine_distance(&query_embedding))
            .limit(limit)
            .select((
                FileChunk::as_select(),
                (1.0.into_sql::<diesel::sql_types::Double>()
                    - dsl::embedding.cosine_distance(&query_embedding)),
            ))
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(chunks
            .into_iter()
            .map(|(chunk, score)| ScoredFileChunk { chunk, score })
            .collect())
    }

    async fn count_file_chunks(&mut self, file_id: Uuid) -> PgResult<i64> {
        use schema::file_chunks::{self, dsl};

        let count: i64 = file_chunks::table
            .filter(dsl::file_id.eq(file_id))
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }
}
