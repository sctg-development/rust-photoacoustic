// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Tests unitaires pour le module create_token refactorisé

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Crée un fichier de configuration temporaire pour les tests
    fn create_test_config() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yaml");

        let config_content = r#"
visualization:
  hmac_secret: "test_secret_key_for_hmac_algorithm"
  rs256_private_key: "LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUlKS0FJQkFBS0NBZ0VBMTUyWUZHNjZocldrQXVrcHNPazlGZXhhYXl0Smp6cXJyZFl0azZRZDFFdkxZRFZKCmxEVkVaN0ZHUXIvczVZUEo5NE9rT3VTaktVT0Z0NHJCaU1IUy85M1N4eUFxNjZvTmU4N0pLS01vQXlTMXhmc0sKNVV5VmpZUTZsOHAxQ1F5MTdHZFNxMWdnZjZqNFZaazFJdUhZQkRDWGdCTVhDdlZvY3p5bkY0SzFORzdJendNMApJcDhOdnIvZHNHZEwzSHJLWkFXTDkxem9kZFdGSkh2K1RkMnUyd2luai8rejc2Mm9LUmhQWTZWVjZiUzlZSVVCCjJoRXl0MkFZbm40Q00rTDNSRm1zRnM5ZkpUNmVKUEFUWmRBVHlBKzk3ZXQ1WFAxU0JTWUFia1lqYjF2OWd3RUIKZ251bmNIbHVTYjdic05ET2JmQTVnbzQ5cDRDWVRhY2VYd1NBZHJwZS9mbFpQd3dzcHFWK294OHhXdjBLVHc3YQovQk9wNUhBL2FMRjRoNHM4RUJJLzFuZ1lCZUdvZ2c2cEYrRnc0WjRnaW5PTVFFN1ptd0p5K1BDU3hDMmMyTkZrClRTQ2k5VFUzU1V1MHoxbzNhSExLN2VZWkoxcTRZU0xzVWpjTUI5KzV6eWxpY3J5aFFsd0FvTTZjeDB4YzBZRVkKU20zN1UrajN2bGhmYTEwRnA1MS9WdVlSQUdrNC9rWWoyYkk3czhWTFNWekhqcXF5ZnNRUHhYTmFZSXZUOFVvcgp2OHdiRk52RDBsMnBadTJEeTM3MWJaMnRaTnZEeDd4dmcvbHFBWUtGemxuUmJrbnNtb3pkdDhCOGd0emsxamFDCk9FZDJOeW1iYzcwbjgyZktWeFdSNW1KVEV6S1dOb3pzYWdKczRsWURBay80aDd4QzFzdC9HR3ZRV3c4Q0F3RUEKQVFLQ0FnQVRtVEtLb09uNWRxYzYwSURHb055NWttdEJsSVN5TFM0UHRna2NnMjFtcjZFRDFMUWtjWmNQS2REdQpIazNsS3M1LzNncGVoQXZFbzJ1VEhGeXRGcGtjUXNoMjZ4aWJwVEJta1l3OVVsOC9zZVdING1MQ0p1enRHUmpPClZVdkFEOVMxY1VyVllrUko5a3prVXZHK2d2TEVwcm9PblMyYUJHYURHdjlCSnROYnVib3MvdWlOUVJIWnhjemIKelBmYlNabjk3M2NpZStKeFc0QW5xZEdhdUV5OWdoTGhCdWdQSUNUSjMzalA0T1puUU5ONTQzMGdtakdXODNncApaK0RCWUo4REtuZDI5MUI1clRCb3dSMnlRbkNaM2J4dzFOZUtPVWhzU1dEK3BRZHhodFlMUTFza3hpS1pHYUNKCnVYeDBuUFkyamdCY09wdW1EdzFJK2FqTUVEZXc2RmZlV215VXNGK0pRbUpRS2E2aUVPeDhBNG5ISTU4ellBdjYKYmFTZFdHU0JBN0lONEMwZDJCdWVTNTl4TUFUZ2o2NFc1UzdOTnlvR3lrTGdRMDlQcFdIR1hqVGh1M2N4NFRUbAo0UlZvRDY5dTBMNEVPbDBIK1dhQVVjRVExQVlIemVob0pIQWFYdUNGVXdxL3R2TTVWVDdjQzl4WVE4bXJTVVcyCnNyQWkvZG1DK1VMS1FUMk4rME1vTlRnRnBqVElDd3dkQVljUmRwa0hpTG9HWnlXV0Z6UERwM0c1b2o1d05PRHkKUzJBSUo1UVAwZWFhb21VUGpNdC9lcFV2Vi9zRkU1dnd2WWo3WDIremdublp6RmRRSjZPZ0Z2d2dQblI5a1kxMwpHTWRjb0hTbzErZkVEYW56YS92b2oxTDg5YjVYb3F3cVYvSEpEZC8zcXVrb3ZiVHJJUUtDQVFFQTYzTU5RVHl6CnlCSWQ3dGN2dHV5S0ppR0h3N2lzemptblRxbGk5ZzlFVmVHUUhsbDBXZURZY3pVMzdMai8vd280RXNKc3RQK04KQ2U2dExuOEJEOTVmbUtnR0xSQmJUVEZxbDEwdTBzMDNubUtZOWwrQnRySi9YUlpLeHVybEVCb1BWakQwamdjVQo0d3NJZjNGOG1pWHhrc2RBVHB6T2d5QVlJcFhadjRtYk9rOXpkYlFBTjAySWhVd3JqRk1ab2V5Y1ZIWk90aUxxClBOdGRZcXB6UzhoRitmemsya01VbndGTFpTZTh0SVk2QWtoK2lxTS9aOWxDYUZzbzZqSlkrQml5dVEzMVoxRFQKcUJOUHB1U3pMV0Q5dCtkQkMyNFVaekVsMHhvdGVSQkVQK09kY1ZhY2NmZlNOWkRLQVZ3YmllUjJ1ZHB6c1RZcQpaY28xWUtFRFZWOEl2d0tDQVFFQTZtOWVyY0huMWQwdW1WMGZ4MkZublJXd2p4WDlZeW1McmFBTzIrWHV1RTNECkJ4Qk5Udkw1enlxYUpCa09hUDZ1Z1dtYjBmeTMwM1JIS08yZzQ1b3VKV3RGanBKdXNnTXE4Qkp3Z2lIR0wzUncKbHlnMkpsSjhrVndNK3QzR2ZocVcyOGRzUVM5L2FmeC9QYzdCSGtxN0QzdDRFYTBIUjB3MTd0SlE0RGtrSnREUQpDTWJZMmdFVm5YdlNsS3pxUVVnVGM1TmF3ZmpKZk5FOHVRcEFodytVWG5MV2ZyZ0VtNVhnM3ZJUngzYW5Ccy9qCnpBa1p3UjlpdEVGZmJoOVl4QnNvUVRIZXBIanFZQTBxVU9EUHRTTjkrRU50bXZtemJlejMreU93MVNlOVFtZUIKNFE2NTA1QXg2RFVQUExtQnkwb0JQMEY3VWQ4K1hoNnBTWHZRZG9GeHNRS0NBUUVBMTBGY1FPUktTUU9uTWhDeApvcjhtbmkzUmZYSjlLaDB6aElyLzJvMmlvQkdVUE9yem9LZ2J1MWhRUksvdCt6RlFpbllPQkh3T2FhSTMybVpxCmhpTjdxb2tTL0dnVDBNTDQvR0ZJeVppczNMU3Z1NjhkVy9aYzBySHFzMmxVK2grWkhlZXI3WjB6ejA3cStmaTcKUGdLcWxOSnRUSEczcTlIUHZ1N1pJRytoNXZGMFVZdTdGdFJmbDk1SGdnY0hUQzZSemZaTGgrRHIzYTkvOVJCNApVRFJOSlh1N2puLzlmbEVrcU5wbmYzT24zU2FCNmlYTmRoZit5b0Z0S28vVkh4MFZhSElHaGVvelYxb3dYQmlDCloxNWhGNXpvcnBaNU9NNktFakhBbVFueUc2Ync3Z21OQmZUWHpma05kYWpMMUlsMnNmdXlBYlhQbFRnRkRNNm4KeVlrVTZRS0NBUUJiRjRVcGJQUGhWTDA3bUVTMTJ6SkFobUlCWnlENisvU3JOVXN3eEtvNmNQUzc5T2lsS2FKSgpqaHN3dFkrMDJta0NIZ0FPMnV5dXZEcStPMzlOa09ZbllnUTMvc3dHWFZhOHk5MXRveVAzNG4yeExrM0VIRDhNCjJFQ3U1YWV2N2pMNkdRWUdXaWRmRGw1K3JLeTdVWHhYNnJqZjNXUzdubkJDYVBSRis4NEJTeGZhb1RFM25ENUMKNEE4VitBNVo2V0N0Z3U4NythaUkyR0NJNGVQcTF2SG10U2FGUEltRnp2bitodjdEaTJZaExud3NGc2tzRWo3dgpIWXV3Slh2L1R6SVlDd0dnMU43MURZaUl2cUhXbDREbUM0VTJONW94dDJjdjdWRlRzY3BIV0NMT1NVT0pZamtTCktqUE9lNkprVkZBaHZSYm0zQ0RHdjVFMTNXZEY5TXd4QW9JQkFDQzEvWkY5MXlCK0ZrTFVNK1MrVW8ydVVzUGIKVzZuTXFaMTNrNVh1V3JDay8rSGhBZkNTZityTm5uYUg5TUd4MzNGbktpVGpraHVwM2RuOVQyVVRTcnVXaG9uZgo5bXVrdUJzazJBM0lZOU1PaDVLTW1tNUw2NkYxQ09lNnRBc1FrUmo4aUowejNMK1RGdDg1ZnU3eitmeGlqakZSCmR1bzRqTjVoazVMdTViTTlhcnBXKzUwSUw2UkNCbDVwZ2pQV1dxZFJ2Z3FBWWtIcjVhVUFGbSs1Vjh0VzNJV2EKNzdFUk9mOEIrYm54OEEraVAremFYRWY4NXRzQ0JqUDc3VHU4aU1qbFh2amFIdHltTGRsV2E1NXdpaGx5L3dLTQpIU05KWTJ2VXVRSjR3WHlrR3pGTWtkTjI3TVZHWmk1enVoV3hacjlZWEZGY1hWTWtMdnBXTys5TThzZz0KLS0tLS1FTkQgUlNBIFBSSVZBVEUgS0VZLS0tLS0K"  # Base64 encoded test key
  rs256_public_key: "LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1JSUNDZ0tDQWdFQTE1MllGRzY2aHJXa0F1a3BzT2s5RmV4YWF5dEpqenFycmRZdGs2UWQxRXZMWURWSmxEVkUKWjdGR1FyL3M1WVBKOTRPa091U2pLVU9GdDRyQmlNSFMvOTNTeHlBcTY2b05lODdKS0tNb0F5UzF4ZnNLNVV5VgpqWVE2bDhwMUNReTE3R2RTcTFnZ2Y2ajRWWmsxSXVIWUJEQ1hnQk1YQ3ZWb2N6eW5GNEsxTkc3SXp3TTBJcDhOCnZyL2RzR2RMM0hyS1pBV0w5MXpvZGRXRkpIditUZDJ1MndpbmovK3o3NjJvS1JoUFk2VlY2YlM5WUlVQjJoRXkKdDJBWW5uNENNK0wzUkZtc0ZzOWZKVDZlSlBBVFpkQVR5QSs5N2V0NVhQMVNCU1lBYmtZamIxdjlnd0VCZ251bgpjSGx1U2I3YnNORE9iZkE1Z280OXA0Q1lUYWNlWHdTQWRycGUvZmxaUHd3c3BxVitveDh4V3YwS1R3N2EvQk9wCjVIQS9hTEY0aDRzOEVCSS8xbmdZQmVHb2dnNnBGK0Z3NFo0Z2luT01RRTdabXdKeStQQ1N4QzJjMk5Ga1RTQ2kKOVRVM1NVdTB6MW8zYUhMSzdlWVpKMXE0WVNMc1VqY01COSs1enlsaWNyeWhRbHdBb002Y3gweGMwWUVZU20zNwpVK2ozdmxoZmExMEZwNTEvVnVZUkFHazQva1lqMmJJN3M4VkxTVnpIanFxeWZzUVB4WE5hWUl2VDhVb3J2OHdiCkZOdkQwbDJwWnUyRHkzNzFiWjJ0Wk52RHg3eHZnL2xxQVlLRnpsblJia25zbW96ZHQ4QjhndHprMWphQ09FZDIKTnltYmM3MG44MmZLVnhXUjVtSlRFektXTm96c2FnSnM0bFlEQWsvNGg3eEMxc3QvR0d2UVd3OENBd0VBQVE9PQotLS0tLUVORCBSU0EgUFVCTElDIEtFWS0tLS0tCg=="   # Base64 encoded test key

access:
  duration: 3600
  iss: "TestIssuer"
  users:
    - user: "testuser"
      permissions: ["read", "write"]
    - user: "readonly"
      permissions: ["read"]
  clients:
    - client_id: "test_client"
      default_scope: "basic"
      allowed_callbacks: ["http://localhost:3000/callback"]
"#;

        std::fs::write(&config_path, config_content).unwrap();
        (temp_dir, config_path)
    }

    #[test]
    fn test_jwt_algorithm_parsing() {
        assert!(matches!(
            JwtAlgorithm::from_str("HS256").unwrap(),
            JwtAlgorithm::HS256
        ));
        assert!(matches!(
            JwtAlgorithm::from_str("RS256").unwrap(),
            JwtAlgorithm::RS256
        ));
        assert!(JwtAlgorithm::from_str("INVALID").is_err());
    }

    #[test]
    fn test_config_loader_user_validation() {
        let (_temp_dir, config_path) = create_test_config();
        let config_loader = ConfigLoader::load(&config_path).unwrap();

        // Test utilisateur existant
        let user = config_loader.find_user("testuser");
        assert!(user.is_ok());
        assert_eq!(user.unwrap().permissions, vec!["read", "write"]);

        // Test utilisateur inexistant
        let user = config_loader.find_user("nonexistent");
        assert!(user.is_err());
        assert!(matches!(
            user.unwrap_err(),
            TokenCreationError::UserNotFound { .. }
        ));
    }

    #[test]
    fn test_config_loader_client_validation() {
        let (_temp_dir, config_path) = create_test_config();
        let config_loader = ConfigLoader::load(&config_path).unwrap();

        // Test client existant
        let client = config_loader.find_client("test_client");
        assert!(client.is_ok());
        assert_eq!(client.unwrap().client_id, "test_client");

        // Test client inexistant
        let client = config_loader.find_client("nonexistent");
        assert!(client.is_err());
        assert!(matches!(
            client.unwrap_err(),
            TokenCreationError::ClientNotFound { .. }
        ));
    }

    #[test]
    fn test_duration_override() {
        let (_temp_dir, config_path) = create_test_config();
        let config_loader = ConfigLoader::load(&config_path).unwrap();

        // Test sans override
        let duration = config_loader.get_token_duration(None);
        assert_eq!(duration, 3600);

        // Test avec override
        let duration = config_loader.get_token_duration(Some(7200));
        assert_eq!(duration, 7200);
    }

    #[test]
    fn test_error_exit_codes() {
        let error = TokenCreationError::UserNotFound {
            user: "test".to_string(),
            available_users: "user1, user2".to_string(),
        };
        assert_eq!(error.exit_code(), 2);

        let error = TokenCreationError::ClientNotFound {
            client: "test".to_string(),
        };
        assert_eq!(error.exit_code(), 3);
    }
}
