//! Script-related tools
//!
//! Tools for creating and managing Lua scripts/spells.

use std::fs;
use std::path::PathBuf;
use crate::protocol::ToolDefinition;
use super::ToolError;

/// Create a new spell script
pub fn create_spell_definition() -> ToolDefinition {
    ToolDefinition {
        name: "create_spell".to_string(),
        description: "Create a new spell Lua script for SKOPE Engine. Generates a spell file with the specified properties and behavior.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the spell (e.g., 'fireball', 'ice_storm')"
                },
                "damage_type": {
                    "type": "string",
                    "enum": ["fire", "ice", "lightning", "physical", "arcane", "nature", "dark", "light"],
                    "description": "Type of damage the spell deals"
                },
                "base_damage": {
                    "type": "number",
                    "description": "Base damage value of the spell"
                },
                "cooldown": {
                    "type": "number",
                    "description": "Cooldown time in seconds"
                },
                "description": {
                    "type": "string",
                    "description": "Description of the spell's effect"
                },
                "projectile": {
                    "type": "boolean",
                    "description": "Whether the spell fires a projectile"
                },
                "aoe_radius": {
                    "type": "number",
                    "description": "Area of effect radius (0 for single target)"
                }
            },
            "required": ["name", "damage_type", "base_damage"]
        }),
    }
}

/// Create spell implementation
pub fn create_spell(project_path: &PathBuf, args: serde_json::Value) -> Result<String, ToolError> {
    let name = args.get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidArgs("Missing 'name' parameter".to_string()))?;

    let damage_type = args.get("damage_type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidArgs("Missing 'damage_type' parameter".to_string()))?;

    let base_damage = args.get("base_damage")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| ToolError::InvalidArgs("Missing 'base_damage' parameter".to_string()))?;

    let cooldown = args.get("cooldown")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);

    let description = args.get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("A magical spell");

    let projectile = args.get("projectile")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let aoe_radius = args.get("aoe_radius")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    // Generate Lua script
    let script_content = generate_spell_script(
        name,
        damage_type,
        base_damage,
        cooldown,
        description,
        projectile,
        aoe_radius,
    );

    // Write to file
    let scripts_dir = project_path.join("assets").join("scripts").join("spells");
    fs::create_dir_all(&scripts_dir)?;

    let script_path = scripts_dir.join(format!("{}.lua", name));
    fs::write(&script_path, &script_content)?;

    Ok(format!(
        "Created spell '{}' at {}\n\nScript contents:\n```lua\n{}\n```",
        name,
        script_path.display(),
        script_content
    ))
}

fn generate_spell_script(
    name: &str,
    damage_type: &str,
    base_damage: f64,
    cooldown: f64,
    description: &str,
    projectile: bool,
    aoe_radius: f64,
) -> String {
    let effect_name = match damage_type {
        "fire" => "fire_explosion",
        "ice" => "ice_shatter",
        "lightning" => "lightning_strike",
        "arcane" => "arcane_burst",
        "nature" => "nature_bloom",
        "dark" => "dark_void",
        "light" => "holy_radiance",
        _ => "magic_impact",
    };

    let projectile_code = if projectile {
        format!(r#"
-- Projectile settings
local projectile_speed = 20.0
local projectile_model = "spell_{}"

function {}_on_cast(caster, target_pos)
    -- Spawn projectile
    local start_pos = SKOPE.Entity.get_position(caster)
    local direction = SKOPE.Math.normalize(SKOPE.Math.sub(target_pos, start_pos))

    local projectile = SKOPE.Entity.spawn("{}_projectile", start_pos)
    SKOPE.Entity.set_velocity(projectile, SKOPE.Math.scale(direction, projectile_speed))

    -- Play cast sound
    SKOPE.Audio.play_at("spell_cast_{}", start_pos)

    return projectile
end
"#, damage_type, name, name, damage_type)
    } else {
        format!(r#"
function {}_on_cast(caster, target_pos)
    -- Instant cast at target position
    SKOPE.VFX.spawn("{}", target_pos)
    SKOPE.Audio.play_at("spell_cast_{}", target_pos)

    -- Apply damage immediately
    {}_apply_damage(caster, target_pos)
end
"#, name, effect_name, damage_type, name)
    };

    let aoe_code = if aoe_radius > 0.0 {
        format!(r#"
function {}_apply_damage(caster, position)
    -- Find all entities in AOE
    local targets = SKOPE.Physics.overlap_sphere(position, {:.1})

    for _, target in ipairs(targets) do
        if SKOPE.Entity.is_damageable(target) and target ~= caster then
            -- Calculate damage falloff from center
            local dist = SKOPE.Math.distance(position, SKOPE.Entity.get_position(target))
            local falloff = 1.0 - (dist / {:.1})
            local damage = {:.0} * falloff

            SKOPE.Entity.apply_damage(target, damage, "{}")
            SKOPE.VFX.spawn("{}", SKOPE.Entity.get_position(target))
        end
    end
end
"#, name, aoe_radius, aoe_radius, base_damage, damage_type, effect_name)
    } else {
        format!(r#"
function {}_apply_damage(caster, position)
    -- Single target damage
    local target = SKOPE.Physics.raycast(
        SKOPE.Entity.get_position(caster),
        position,
        100.0
    )

    if target and SKOPE.Entity.is_damageable(target) then
        SKOPE.Entity.apply_damage(target, {:.0}, "{}")
        SKOPE.VFX.spawn("{}", SKOPE.Entity.get_position(target))
    end
end
"#, name, base_damage, damage_type, effect_name)
    };

    format!(r#"-- {name}: {description}
-- Damage Type: {damage_type}
-- Base Damage: {base_damage:.0}
-- Cooldown: {cooldown:.1}s
-- AOE Radius: {aoe_radius:.1}

local spell = {{
    name = "{name}",
    damage_type = "{damage_type}",
    base_damage = {base_damage:.0},
    cooldown = {cooldown:.1},
    aoe_radius = {aoe_radius:.1},
}}
{projectile_code}
{aoe_code}
function {name}_on_hit(caster, target, projectile)
    -- Called when projectile hits target
    if SKOPE.Entity.is_damageable(target) then
        {name}_apply_damage(caster, SKOPE.Entity.get_position(target))
    end

    -- Destroy projectile if it exists
    if projectile then
        SKOPE.Entity.destroy(projectile)
    end
end

function {name}_on_end(caster)
    -- Called when spell ends (for channeled/over-time effects)
    SKOPE.Debug.log("{name} effect ended")
end

-- Register spell with SKOPE
SKOPE.Spell.register("{name}", {{
    on_cast = {name}_on_cast,
    on_hit = {name}_on_hit,
    on_end = {name}_on_end,
    cooldown = spell.cooldown,
    damage_type = spell.damage_type,
}})

return spell
"#,
        name = name,
        description = description,
        damage_type = damage_type,
        base_damage = base_damage,
        cooldown = cooldown,
        aoe_radius = aoe_radius,
        projectile_code = projectile_code,
        aoe_code = aoe_code,
    )
}

/// List scripts definition
pub fn list_scripts_definition() -> ToolDefinition {
    ToolDefinition {
        name: "list_scripts".to_string(),
        description: "List all Lua scripts in the SKOPE project".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// List scripts implementation
pub fn list_scripts(project_path: &PathBuf) -> Result<String, ToolError> {
    let scripts_dir = project_path.join("assets").join("scripts");

    if !scripts_dir.exists() {
        return Ok("No scripts directory found".to_string());
    }

    let mut scripts = Vec::new();
    collect_lua_files(&scripts_dir, &scripts_dir, &mut scripts)?;

    if scripts.is_empty() {
        return Ok("No Lua scripts found".to_string());
    }

    let mut output = format!("Found {} Lua scripts:\n\n", scripts.len());
    for (path, size) in scripts {
        output.push_str(&format!("- {} ({} bytes)\n", path, size));
    }

    Ok(output)
}

fn collect_lua_files(base: &PathBuf, dir: &PathBuf, scripts: &mut Vec<(String, u64)>) -> Result<(), ToolError> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_lua_files(base, &path, scripts)?;
        } else if path.extension().map_or(false, |e| e == "lua") {
            let relative = path.strip_prefix(base)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.display().to_string());
            let size = entry.metadata()?.len();
            scripts.push((relative, size));
        }
    }

    Ok(())
}

/// Reload script definition
pub fn reload_script_definition() -> ToolDefinition {
    ToolDefinition {
        name: "reload_script".to_string(),
        description: "Signal the SKOPE engine to reload a specific script (requires running engine with hot-reload enabled)".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path to the script (e.g., 'spells/fireball.lua')"
                }
            },
            "required": ["path"]
        }),
    }
}

/// Reload script implementation
pub fn reload_script(project_path: &PathBuf, args: serde_json::Value) -> Result<String, ToolError> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidArgs("Missing 'path' parameter".to_string()))?;

    let script_path = project_path.join("assets").join("scripts").join(path);

    if !script_path.exists() {
        return Err(ToolError::ExecutionError(format!(
            "Script not found: {}",
            script_path.display()
        )));
    }

    // Touch the file to trigger hot-reload (if file watcher is active)
    let now = std::time::SystemTime::now();
    if let Err(_) = filetime::set_file_mtime(&script_path, filetime::FileTime::from_system_time(now)) {
        // filetime might not be available, try alternative
        let content = fs::read_to_string(&script_path)?;
        fs::write(&script_path, content)?;
    }

    Ok(format!(
        "Reload signal sent for: {}\n\nNote: The SKOPE engine must be running with hot-reload enabled to pick up changes.",
        path
    ))
}
