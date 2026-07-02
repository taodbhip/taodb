//! 多用户 + 多项目隔离
//!
//! 商业模式核心：每个用户有多个项目，每个项目独立的数据 + 配置
//!
//! Tenant 层级：
//!   - System（root，admin）
//!   - User（个人开发者 / 网文作者）
//!   - Project（用户下的具体项目，如"chudao"、"tjworld"）
//!
//! 路径：
//!   {data_dir}/users/{user_id}/projects/{project_id}/db
//!
//! API token：
//!   每个 user 一个 API token，每个 project 一个可选的 write token

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub user_id: String,
    pub project_id: String,
}

/// 用户配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserConfig {
    pub user_id: String,
    pub email: String,
    pub tier: String, // "free" | "standard" | "pro"
    pub api_token: String,
    pub created_at: i64,
    pub storage_quota_mb: u64,     // 存储配额
    pub monthly_recall_limit: u64, // 月召回限额
}

/// 项目配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    pub project_id: String,
    pub user_id: String,
    pub name: String,
    pub description: String,
    pub created_at: i64,
}

/// 租户管理器
pub struct TenantManager {
    data_dir: PathBuf,
}

impl TenantManager {
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Self {
        Self {
            data_dir: data_dir.as_ref().to_path_buf(),
        }
    }

    /// 创建用户
    pub fn create_user(&self, user_id: &str, email: &str) -> Result<UserConfig> {
        let user_dir = self.data_dir.join("users").join(user_id);
        std::fs::create_dir_all(&user_dir)?;
        let cfg = UserConfig {
            user_id: user_id.to_string(),
            email: email.to_string(),
            tier: "free".into(),
            api_token: generate_token(),
            created_at: chrono::Utc::now().timestamp(),
            storage_quota_mb: 100,
            monthly_recall_limit: 1000,
        };
        let path = user_dir.join("config.json");
        std::fs::write(&path, serde_json::to_string_pretty(&cfg)?)?;
        Ok(cfg)
    }

    /// 创建项目
    pub fn create_project(&self, user_id: &str, project_id: &str, name: &str) -> Result<ProjectConfig> {
        let project_dir = self.user_dir(user_id).join("projects").join(project_id);
        std::fs::create_dir_all(&project_dir)?;
        let cfg = ProjectConfig {
            project_id: project_id.to_string(),
            user_id: user_id.to_string(),
            name: name.to_string(),
            description: String::new(),
            created_at: chrono::Utc::now().timestamp(),
        };
        let path = project_dir.join("config.json");
        std::fs::write(&path, serde_json::to_string_pretty(&cfg)?)?;
        // 创建子目录
        std::fs::create_dir_all(project_dir.join("db"))?;
        std::fs::create_dir_all(project_dir.join("docs"))?;
        std::fs::create_dir_all(project_dir.join("cache"))?;
        Ok(cfg)
    }

    /// 列出用户的所有项目
    pub fn list_projects(&self, user_id: &str) -> Result<Vec<ProjectConfig>> {
        let projects_dir = self.user_dir(user_id).join("projects");
        if !projects_dir.exists() {
            return Ok(Vec::new());
        }
        let mut projects = Vec::new();
        for entry in std::fs::read_dir(&projects_dir)? {
            let entry = entry?;
            let path = entry.path().join("config.json");
            if path.exists()
                && let Ok(data) = std::fs::read_to_string(&path)
                && let Ok(cfg) = serde_json::from_str::<ProjectConfig>(&data)
            {
                projects.push(cfg);
            }
        }
        Ok(projects)
    }

    /// 列出所有用户
    pub fn list_users(&self) -> Result<Vec<UserConfig>> {
        let users_dir = self.data_dir.join("users");
        if !users_dir.exists() {
            return Ok(Vec::new());
        }
        let mut users = Vec::new();
        for entry in std::fs::read_dir(&users_dir)? {
            let entry = entry?;
            let path = entry.path().join("config.json");
            if path.exists()
                && let Ok(data) = std::fs::read_to_string(&path)
                && let Ok(cfg) = serde_json::from_str::<UserConfig>(&data)
            {
                users.push(cfg);
            }
        }
        Ok(users)
    }

    /// 获取用户
    pub fn get_user(&self, user_id: &str) -> Result<UserConfig> {
        let path = self.user_dir(user_id).join("config.json");
        let data = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&data)?)
    }

    /// 获取项目
    pub fn get_project(&self, user_id: &str, project_id: &str) -> Result<ProjectConfig> {
        let path = self
            .user_dir(user_id)
            .join("projects")
            .join(project_id)
            .join("config.json");
        let data = std::fs::read_to_string(&path).context("project not found")?;
        Ok(serde_json::from_str(&data)?)
    }

    /// 通过 API token 找到用户
    pub fn find_user_by_token(&self, token: &str) -> Result<UserConfig> {
        for user in self.list_users()? {
            if user.api_token == token {
                return Ok(user);
            }
        }
        anyhow::bail!("invalid api token")
    }

    /// 获取项目数据目录
    pub fn project_db_path(&self, user_id: &str, project_id: &str) -> PathBuf {
        self.user_dir(user_id).join("projects").join(project_id).join("db")
    }

    /// 用户目录
    pub fn user_dir(&self, user_id: &str) -> PathBuf {
        self.data_dir.join("users").join(user_id)
    }
}

/// 生成 token — uses OS CSPRNG via rand::rng()
fn generate_token() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let a: u64 = rng.random();
    let b: u64 = rng.random();
    format!("tk_{:016x}_{:016x}", a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    #[test]
    fn create_user_and_project() {
        let dir = TempDir::new("taodb-tenant-test").unwrap();
        let mgr = TenantManager::new(dir.path());
        let user = mgr.create_user("alice", "alice@example.com").unwrap();
        assert!(!user.api_token.is_empty());

        let proj = mgr.create_project("alice", "chudao", "触道归墟").unwrap();
        assert_eq!(proj.project_id, "chudao");

        let projects = mgr.list_projects("alice").unwrap();
        assert_eq!(projects.len(), 1);

        let found = mgr.find_user_by_token(&user.api_token).unwrap();
        assert_eq!(found.user_id, "alice");

        let db_path = mgr.project_db_path("alice", "chudao");
        assert!(db_path.to_string_lossy().contains("chudao"));
        println!("✓ 多用户+多项目隔离工作");
    }

    #[test]
    fn tenant_isolation() {
        let dir = TempDir::new("taodb-tenant-iso").unwrap();
        let mgr = TenantManager::new(dir.path());
        mgr.create_user("alice", "alice@x.com").unwrap();
        mgr.create_user("bob", "bob@x.com").unwrap();

        mgr.create_project("alice", "p1", "P1").unwrap();
        mgr.create_project("bob", "p2", "P2").unwrap();

        // alice 看不到 bob 的项目
        let alice_projects = mgr.list_projects("alice").unwrap();
        let bob_projects = mgr.list_projects("bob").unwrap();
        assert_eq!(alice_projects.len(), 1);
        assert_eq!(bob_projects.len(), 1);
        assert_eq!(alice_projects[0].project_id, "p1");
        assert_eq!(bob_projects[0].project_id, "p2");
        println!("✓ 租户隔离：alice 看不到 bob 的项目");
    }
}
