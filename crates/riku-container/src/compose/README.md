# compose

## Overview
Component package located at `crates/riku-container/src/compose`.

## Interface
- `parse_build` [build.rs:parse_build] (fn)
- `ServiceContext` [definition.rs:ServiceContext] (struct)
- `BuildDef` [definition.rs:BuildDef] (struct)
- `DependsCondition` [definition.rs:DependsCondition] (enum)
- `definition.rs` [definition.rs] (and 2 additional symbols)
- `parse_depends_on` [depends.rs:parse_depends_on] (fn)
- `SCANNED_BYTES` [dotenv_value.rs:SCANNED_BYTES] (static)
- `OpenQuote` [dotenv_value.rs:OpenQuote] (enum)
- `after_first_line` [dotenv_value.rs:after_first_line] (fn)
- `dotenv_value.rs` [dotenv_value.rs] (and 3 additional symbols)
- `parse_duration` [duration.rs:parse_duration] (fn)
- `MAX_APP_FILE_BYTES` [environment.rs:MAX_APP_FILE_BYTES] (const)
- `read_dotenv` [environment.rs:read_dotenv] (fn)
- `read_app_file` [environment.rs:read_app_file] (fn)
- `environment.rs` [environment.rs] (and 2 additional symbols)
- `parse_image` [fields.rs:parse_image] (fn)
- `parse_command` [fields.rs:parse_command] (fn)
- `parse_user` [fields.rs:parse_user] (fn)
- `fields.rs` [fields.rs] (and 5 additional symbols)
- `parse_healthcheck` [health.rs:parse_healthcheck] (fn)
- `TooLarge` [interpolate.rs:TooLarge] (struct)
- `interpolate_capped` [interpolate.rs:interpolate_capped] (fn)
- `SERVICE_KEYS` [keys.rs:SERVICE_KEYS] (const)
- `check_keys` [keys.rs:check_keys] (fn)
- `COMPOSE_FILE_NAMES` [locate.rs:COMPOSE_FILE_NAMES] (const)
- `ComposeFile` [locate.rs:ComposeFile] (struct)
- `arg` [locate.rs:arg] (fn)
- `locate.rs` [locate.rs] (and 3 additional symbols)
- `Mapping` [mod.rs:Mapping] (type)
- `check_bind_source` [mod.rs:check_bind_source] (re-export)
- `Value` [mod.rs:Value] (re-export)
- `mod.rs` [mod.rs] (and further re-exports)
- `VolumeKey` [mounts.rs:VolumeKey] (struct)
- `declared` [mounts.rs:declared] (fn)
- `as_str` [mounts.rs:as_str] (fn)
- `mounts.rs` [mounts.rs] (and 4 additional symbols)
- `start_order` [order.rs:start_order] (fn)
- `resolve_inside` [paths.rs:resolve_inside] (fn)
- `check_bind_source` [paths.rs:check_bind_source] (fn)
- `host_ip_for` [ports.rs:host_ip_for] (fn)
- `parse_ports` [ports.rs:parse_ports] (fn)
- `VolumeDecl` [project.rs:VolumeDecl] (enum)
- `ComposeProject` [project.rs:ComposeProject] (struct)
- `project.rs` [project.rs] (and 2 additional symbols)
- `RawProject` [raw.rs:RawProject] (struct)
- `read_raw` [raw.rs:read_raw] (fn)
- `RoutedPort` [rules.rs:RoutedPort] (struct)
- `ReplaceOrder` [rules.rs:ReplaceOrder] (enum)
- `routed_http` [rules.rs:routed_http] (fn)
- `rules.rs` [rules.rs] (and 2 additional symbols)
- `scalar_text` [scalar.rs:scalar_text] (fn)
- `check_text` [scalar.rs:check_text] (fn)
- `text_value` [scalar.rs:text_value] (fn)
- `scalar.rs` [scalar.rs] (and 4 additional symbols)
- `parse_service` [service.rs:parse_service] (fn)
- `TestApp` [test_support.rs:TestApp] (struct, test builds only)
- `ServiceFixture` [test_support.rs:ServiceFixture] (struct, test builds only)
- `parse_environment` [service_env.rs:parse_environment] (fn)
- `Step` [service_steps.rs:Step] (type)
- `STEPS` [service_steps.rs:STEPS] (const)
- `parse_volume_decls` [volume_decl.rs:parse_volume_decls] (fn)
- `parse_volumes` [volumes.rs:parse_volumes] (fn)

## Invariants
- [definition.rs] Validates and sanitizes identifiers against path traversal and injection
- [locate.rs] Validates and sanitizes identifiers against path traversal and injection
- [mod.rs] Validates and sanitizes identifiers against path traversal and injection
- [ports.rs] Refuses a port that names a host IP; the service's `riku.public` label alone decides loopback versus all interfaces
- [project.rs] Validates and sanitizes identifiers against path traversal and injection

## Side Effects
- None

## Verification
- Run: `cargo test -p riku-container`
