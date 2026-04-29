pub mod devices;
pub mod workspace;

/// Implements `ActiveModelBehavior` with UUID v7 auto-id and `created_at`/`updated_at` auto-fill.
macro_rules! impl_timestamped_behavior {
    ($am:ty) => {
        #[async_trait::async_trait]
        impl sea_orm::ActiveModelBehavior for $am {
            fn new() -> Self {
                Self {
                    id: sea_orm::Set(sea_orm::prelude::Uuid::now_v7()),
                    ..sea_orm::ActiveModelTrait::default()
                }
            }

            async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, sea_orm::DbErr>
            where
                C: sea_orm::ConnectionTrait,
            {
                let now = chrono::Utc::now();
                if insert {
                    self.created_at = sea_orm::Set(now);
                }
                self.updated_at = sea_orm::Set(now);
                Ok(self)
            }
        }
    };
}

pub(crate) use impl_timestamped_behavior;
