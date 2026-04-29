pub use sea_orm_migration::prelude::*;

mod m20260321_000001_init_devices;
mod m20260321_000002_init_workspace;
mod m20260330_000003_uuid_stabilization;
mod m20260331_000004_rel_path_unique;
mod m20260401_000005_datetime_text_workspace;
mod m20260401_000006_datetime_text_devices;
mod m20260407_000007_add_device_name;

pub struct DevicesMigrator;

#[async_trait::async_trait]
impl MigratorTrait for DevicesMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260321_000001_init_devices::Migration),
            Box::new(m20260401_000006_datetime_text_devices::Migration),
            Box::new(m20260407_000007_add_device_name::Migration),
        ]
    }
}

pub struct WorkspaceMigrator;

#[async_trait::async_trait]
impl MigratorTrait for WorkspaceMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260321_000002_init_workspace::Migration),
            Box::new(m20260330_000003_uuid_stabilization::Migration),
            Box::new(m20260331_000004_rel_path_unique::Migration),
            Box::new(m20260401_000005_datetime_text_workspace::Migration),
        ]
    }
}
