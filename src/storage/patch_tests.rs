#[cfg(test)]
mod tests {
    use crate::storage::{SqliteStorage, Storage};
    use std::sync::Arc;

    async fn setup_sqlite() -> Arc<dyn Storage> {
        let storage = SqliteStorage::new("sqlite::memory:", 5).await.unwrap();
        storage.init().await.unwrap();
        Arc::new(storage)
    }

    async fn create_test_urls(storage: &Arc<dyn Storage>) {
        // Create URL with normal user
        storage
            .create_with_code("normal1", "https://example.com/1", Some("user123"))
            .await
            .unwrap();

        // Create URL with all-zero UUID (malformed)
        storage
            .create_with_code(
                "malformed1",
                "https://example.com/2",
                Some("00000000-0000-0000-0000-000000000000"),
            )
            .await
            .unwrap();

        // Create URL with empty string (malformed)
        storage
            .create_with_code("malformed2", "https://example.com/3", Some(""))
            .await
            .unwrap();

        // Create URL with null created_by (malformed)
        storage
            .create_with_code("malformed3", "https://example.com/4", None)
            .await
            .unwrap();

        // Create another normal URL
        storage
            .create_with_code("normal2", "https://example.com/5", Some("user456"))
            .await
            .unwrap();

        // Create another all-zero UUID URL
        storage
            .create_with_code(
                "malformed4",
                "https://example.com/6",
                Some("00000000-0000-0000-0000-000000000000"),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_patch_created_by_single_url() {
        let storage = setup_sqlite().await;
        create_test_urls(&storage).await;

        // Patch a single URL
        let updated = storage
            .patch_created_by("malformed1", "newuser789")
            .await
            .unwrap();
        assert!(updated, "Should have updated the URL");

        // Verify the patch
        let url = storage.get_authoritative("malformed1").await.unwrap();
        assert!(url.is_some());
        assert_eq!(url.unwrap().created_by, Some("newuser789".to_string()));

        // Verify other URLs are unchanged
        let url2 = storage.get_authoritative("normal1").await.unwrap();
        assert_eq!(url2.unwrap().created_by, Some("user123".to_string()));
    }

    #[tokio::test]
    async fn test_patch_created_by_nonexistent_url() {
        let storage = setup_sqlite().await;

        // Try to patch a URL that doesn't exist
        let updated = storage
            .patch_created_by("nonexistent", "newuser789")
            .await
            .unwrap();
        assert!(!updated, "Should not have updated nonexistent URL");
    }

    #[tokio::test]
    async fn test_patch_all_malformed_created_by() {
        let storage = setup_sqlite().await;
        create_test_urls(&storage).await;

        // Patch all malformed URLs
        let count = storage
            .patch_all_malformed_created_by("fixeduser")
            .await
            .unwrap();
        
        // Should have patched 4 malformed entries (2 all-zero UUID, 1 empty string, 1 null)
        assert_eq!(count, 4, "Should have patched exactly 4 malformed URLs");

        // Verify malformed URLs are now fixed
        let url1 = storage.get_authoritative("malformed1").await.unwrap();
        assert_eq!(url1.unwrap().created_by, Some("fixeduser".to_string()));

        let url2 = storage.get_authoritative("malformed2").await.unwrap();
        assert_eq!(url2.unwrap().created_by, Some("fixeduser".to_string()));

        let url3 = storage.get_authoritative("malformed3").await.unwrap();
        assert_eq!(url3.unwrap().created_by, Some("fixeduser".to_string()));

        let url4 = storage.get_authoritative("malformed4").await.unwrap();
        assert_eq!(url4.unwrap().created_by, Some("fixeduser".to_string()));

        // Verify normal URLs are unchanged
        let normal1 = storage.get_authoritative("normal1").await.unwrap();
        assert_eq!(normal1.unwrap().created_by, Some("user123".to_string()));

        let normal2 = storage.get_authoritative("normal2").await.unwrap();
        assert_eq!(normal2.unwrap().created_by, Some("user456".to_string()));
    }

    #[tokio::test]
    async fn test_patch_all_malformed_no_malformed_urls() {
        let storage = setup_sqlite().await;

        // Create only normal URLs
        storage
            .create_with_code("normal1", "https://example.com/1", Some("user123"))
            .await
            .unwrap();
        storage
            .create_with_code("normal2", "https://example.com/2", Some("user456"))
            .await
            .unwrap();

        // Try to patch all malformed URLs
        let count = storage
            .patch_all_malformed_created_by("fixeduser")
            .await
            .unwrap();
        
        assert_eq!(count, 0, "Should have patched 0 URLs when all are normal");

        // Verify URLs are unchanged
        let url1 = storage.get_authoritative("normal1").await.unwrap();
        assert_eq!(url1.unwrap().created_by, Some("user123".to_string()));

        let url2 = storage.get_authoritative("normal2").await.unwrap();
        assert_eq!(url2.unwrap().created_by, Some("user456".to_string()));
    }

    #[tokio::test]
    async fn test_patch_does_not_overwrite_valid_uuids() {
        let storage = setup_sqlite().await;

        // Create URLs with valid UUIDs and other user IDs
        storage
            .create_with_code(
                "valid_uuid",
                "https://example.com/1",
                Some("123e4567-e89b-12d3-a456-426614174000"),
            )
            .await
            .unwrap();

        storage
            .create_with_code(
                "email_user",
                "https://example.com/2",
                Some("user@example.com"),
            )
            .await
            .unwrap();

        storage
            .create_with_code("simple_id", "https://example.com/3", Some("admin"))
            .await
            .unwrap();

        // Create one malformed URL
        storage
            .create_with_code(
                "malformed",
                "https://example.com/4",
                Some("00000000-0000-0000-0000-000000000000"),
            )
            .await
            .unwrap();

        // Patch all malformed URLs
        let count = storage
            .patch_all_malformed_created_by("fixeduser")
            .await
            .unwrap();
        
        assert_eq!(count, 1, "Should have patched only 1 malformed URL");

        // Verify valid UUIDs and other IDs are unchanged
        let url1 = storage.get_authoritative("valid_uuid").await.unwrap();
        assert_eq!(
            url1.unwrap().created_by,
            Some("123e4567-e89b-12d3-a456-426614174000".to_string())
        );

        let url2 = storage.get_authoritative("email_user").await.unwrap();
        assert_eq!(
            url2.unwrap().created_by,
            Some("user@example.com".to_string())
        );

        let url3 = storage.get_authoritative("simple_id").await.unwrap();
        assert_eq!(url3.unwrap().created_by, Some("admin".to_string()));

        // Verify malformed URL is fixed
        let url4 = storage.get_authoritative("malformed").await.unwrap();
        assert_eq!(url4.unwrap().created_by, Some("fixeduser".to_string()));
    }

    #[tokio::test]
    async fn test_patch_single_url_updates_only_target() {
        let storage = setup_sqlite().await;

        // Create multiple malformed URLs
        storage
            .create_with_code(
                "malformed1",
                "https://example.com/1",
                Some("00000000-0000-0000-0000-000000000000"),
            )
            .await
            .unwrap();

        storage
            .create_with_code("malformed2", "https://example.com/2", None)
            .await
            .unwrap();

        storage
            .create_with_code("malformed3", "https://example.com/3", Some(""))
            .await
            .unwrap();

        // Patch only one URL
        let updated = storage
            .patch_created_by("malformed2", "specificuser")
            .await
            .unwrap();
        assert!(updated);

        // Verify only the targeted URL is updated
        let url1 = storage.get_authoritative("malformed1").await.unwrap();
        assert_eq!(
            url1.unwrap().created_by,
            Some("00000000-0000-0000-0000-000000000000".to_string())
        );

        let url2 = storage.get_authoritative("malformed2").await.unwrap();
        assert_eq!(
            url2.unwrap().created_by,
            Some("specificuser".to_string())
        );

        let url3 = storage.get_authoritative("malformed3").await.unwrap();
        assert_eq!(url3.unwrap().created_by, Some("".to_string()));
    }
}
