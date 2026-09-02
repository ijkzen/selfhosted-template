use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Note: 模板示例域资源，演示"列表 + 新建 + 编辑 + 删除"的完整 CRUD 骨架。
/// 新项目以它为母版替换成自己的业务域。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "notes")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,
    pub title: String,
    pub content: String,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
