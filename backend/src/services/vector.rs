use anyhow::{Context, Result};
use qdrant_client::qdrant::{
    CreateCollectionBuilder, DeletePointsBuilder, Distance, PointStruct, QueryPointsBuilder,
    UpsertPointsBuilder, VectorParamsBuilder,
};
use qdrant_client::Qdrant;

use crate::config::QdrantConfig;

pub struct SearchResult {
    pub point_id: String,
    pub score: f32,
    pub content: String,
}

pub struct VectorService {
    client: Qdrant,
    collection_name: String,
    vector_size: u64,
}

impl VectorService {
    pub async fn new(config: &QdrantConfig) -> Result<Self> {
        let client = Qdrant::from_url(&config.url)
            .build()
            .context("Failed to connect to Qdrant")?;

        let service = Self {
            client,
            collection_name: config.collection_name.clone(),
            vector_size: config.vector_size,
        };

        service.ensure_collection().await?;

        Ok(service)
    }

    async fn ensure_collection(&self) -> Result<()> {
        let exists = self
            .client
            .collection_exists(&self.collection_name)
            .await
            .context("Failed to check Qdrant collection")?;

        if !exists {
            self.client
                .create_collection(
                    CreateCollectionBuilder::new(&self.collection_name)
                        .vectors_config(VectorParamsBuilder::new(
                            self.vector_size,
                            Distance::Cosine,
                        )),
                )
                .await
                .context("Failed to create Qdrant collection")?;

            tracing::info!(
                "Created Qdrant collection '{}' (vector_size={})",
                self.collection_name,
                self.vector_size
            );
        }

        Ok(())
    }

    pub async fn upsert_chunks(
        &self,
        chunks: Vec<(String, Vec<f64>, String)>, // (point_id, embedding, content)
    ) -> Result<()> {
        if chunks.is_empty() {
            return Ok(());
        }

        let points: Vec<PointStruct> = chunks
            .into_iter()
            .map(|(id, embedding, content)| {
                let payload: std::collections::HashMap<String, qdrant_client::qdrant::Value> = [(
                    "content".to_string(),
                    qdrant_client::qdrant::Value::from(content),
                )]
                .into();

                // Qdrant expects f32 vectors
                let embedding_f32: Vec<f32> = embedding.iter().map(|&v| v as f32).collect();

                PointStruct::new(id, embedding_f32, payload)
            })
            .collect();

        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection_name, points))
            .await
            .context("Failed to upsert points to Qdrant")?;

        Ok(())
    }

    pub async fn search(
        &self,
        query_embedding: Vec<f64>,
        top_k: u64,
    ) -> Result<Vec<SearchResult>> {
        let query_f32: Vec<f32> = query_embedding.iter().map(|&v| v as f32).collect();
        let response = self
            .client
            .query(
                QueryPointsBuilder::new(&self.collection_name)
                    .query(query_f32)
                    .limit(top_k)
                    .with_payload(true),
            )
            .await
            .context("Failed to search Qdrant")?;

        let results = response
            .result
            .into_iter()
            .map(|point| {
                let point_id = match point.id {
                    Some(ref id) => {
                        use qdrant_client::qdrant::point_id::PointIdOptions;
                        match &id.point_id_options {
                            Some(PointIdOptions::Uuid(uuid)) => uuid.clone(),
                            Some(PointIdOptions::Num(num)) => num.to_string(),
                            None => String::new(),
                        }
                    }
                    None => String::new(),
                };

                let content = point
                    .payload
                    .get("content")
                    .and_then(|v| {
                        // qdrant Value has a kind field with the actual data
                        use qdrant_client::qdrant::value::Kind;
                        match &v.kind {
                            Some(Kind::StringValue(s)) => Some(s.clone()),
                            _ => None,
                        }
                    })
                    .unwrap_or_default();

                SearchResult {
                    point_id,
                    score: point.score,
                    content,
                }
            })
            .collect();

        Ok(results)
    }

    pub async fn delete_points(&self, point_ids: Vec<String>) -> Result<()> {
        if point_ids.is_empty() {
            return Ok(());
        }

        let ids: Vec<qdrant_client::qdrant::PointId> = point_ids
            .into_iter()
            .map(|id| qdrant_client::qdrant::PointId::from(id))
            .collect();

        self.client
            .delete_points(DeletePointsBuilder::new(&self.collection_name).points(ids))
            .await
            .context("Failed to delete points from Qdrant")?;

        Ok(())
    }
}
