//! Math API for Lua
//!
//! Vec3, Quat, Math 관련 Lua API

use mlua::{Lua, Result as LuaResult, Table};

/// Math APIs 등록 (Vec3, Quat, Math)
pub fn register_math_apis(lua: &Lua, skope: &Table) -> LuaResult<()> {
    register_vec3(lua, skope)?;
    register_quat(lua, skope)?;
    register_math(lua, skope)?;
    Ok(())
}

/// Vec3 API
pub fn register_vec3(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let vec3_mt = lua.create_table()?;

    // Vec3.new(x, y, z)
    vec3_mt.set("new", lua.create_function(|lua, (x, y, z): (f32, f32, f32)| {
        let t = lua.create_table()?;
        t.set("x", x)?;
        t.set("y", y)?;
        t.set("z", z)?;
        Ok(t)
    })?)?;

    // Vec3.zero()
    vec3_mt.set("zero", lua.create_function(|lua, ()| {
        let t = lua.create_table()?;
        t.set("x", 0.0)?;
        t.set("y", 0.0)?;
        t.set("z", 0.0)?;
        Ok(t)
    })?)?;

    // Vec3.one()
    vec3_mt.set("one", lua.create_function(|lua, ()| {
        let t = lua.create_table()?;
        t.set("x", 1.0)?;
        t.set("y", 1.0)?;
        t.set("z", 1.0)?;
        Ok(t)
    })?)?;

    // Vec3.up() - Z-up 좌표계 (Blender 호환)
    vec3_mt.set("up", lua.create_function(|lua, ()| {
        let t = lua.create_table()?;
        t.set("x", 0.0)?;
        t.set("y", 0.0)?;
        t.set("z", 1.0)?;  // Z-up
        Ok(t)
    })?)?;

    // Vec3.forward() - Z-up 좌표계 (Blender 호환: -Y가 전방)
    vec3_mt.set("forward", lua.create_function(|lua, ()| {
        let t = lua.create_table()?;
        t.set("x", 0.0)?;
        t.set("y", -1.0)?;  // Z-up: forward is -Y
        t.set("z", 0.0)?;
        Ok(t)
    })?)?;

    // Vec3.add(a, b)
    vec3_mt.set("add", lua.create_function(|lua, (a, b): (Table, Table)| {
        let t = lua.create_table()?;
        t.set("x", a.get::<f32>("x")? + b.get::<f32>("x")?)?;
        t.set("y", a.get::<f32>("y")? + b.get::<f32>("y")?)?;
        t.set("z", a.get::<f32>("z")? + b.get::<f32>("z")?)?;
        Ok(t)
    })?)?;

    // Vec3.sub(a, b)
    vec3_mt.set("sub", lua.create_function(|lua, (a, b): (Table, Table)| {
        let t = lua.create_table()?;
        t.set("x", a.get::<f32>("x")? - b.get::<f32>("x")?)?;
        t.set("y", a.get::<f32>("y")? - b.get::<f32>("y")?)?;
        t.set("z", a.get::<f32>("z")? - b.get::<f32>("z")?)?;
        Ok(t)
    })?)?;

    // Vec3.mul(v, scalar)
    vec3_mt.set("mul", lua.create_function(|lua, (v, s): (Table, f32)| {
        let t = lua.create_table()?;
        t.set("x", v.get::<f32>("x")? * s)?;
        t.set("y", v.get::<f32>("y")? * s)?;
        t.set("z", v.get::<f32>("z")? * s)?;
        Ok(t)
    })?)?;

    // Vec3.length(v)
    vec3_mt.set("length", lua.create_function(|_, v: Table| {
        let x: f32 = v.get("x")?;
        let y: f32 = v.get("y")?;
        let z: f32 = v.get("z")?;
        Ok((x * x + y * y + z * z).sqrt())
    })?)?;

    // Vec3.normalize(v)
    vec3_mt.set("normalize", lua.create_function(|lua, v: Table| {
        let x: f32 = v.get("x")?;
        let y: f32 = v.get("y")?;
        let z: f32 = v.get("z")?;
        let len = (x * x + y * y + z * z).sqrt();
        let t = lua.create_table()?;
        if len > 0.0001 {
            t.set("x", x / len)?;
            t.set("y", y / len)?;
            t.set("z", z / len)?;
        } else {
            t.set("x", 0.0)?;
            t.set("y", 0.0)?;
            t.set("z", 0.0)?;
        }
        Ok(t)
    })?)?;

    // Vec3.dot(a, b)
    vec3_mt.set("dot", lua.create_function(|_, (a, b): (Table, Table)| {
        let ax: f32 = a.get("x")?;
        let ay: f32 = a.get("y")?;
        let az: f32 = a.get("z")?;
        let bx: f32 = b.get("x")?;
        let by: f32 = b.get("y")?;
        let bz: f32 = b.get("z")?;
        Ok(ax * bx + ay * by + az * bz)
    })?)?;

    // Vec3.cross(a, b)
    vec3_mt.set("cross", lua.create_function(|lua, (a, b): (Table, Table)| {
        let ax: f32 = a.get("x")?;
        let ay: f32 = a.get("y")?;
        let az: f32 = a.get("z")?;
        let bx: f32 = b.get("x")?;
        let by: f32 = b.get("y")?;
        let bz: f32 = b.get("z")?;
        let t = lua.create_table()?;
        t.set("x", ay * bz - az * by)?;
        t.set("y", az * bx - ax * bz)?;
        t.set("z", ax * by - ay * bx)?;
        Ok(t)
    })?)?;

    // Vec3.lerp(a, b, t)
    vec3_mt.set("lerp", lua.create_function(|lua, (a, b, t): (Table, Table, f32)| {
        let ax: f32 = a.get("x")?;
        let ay: f32 = a.get("y")?;
        let az: f32 = a.get("z")?;
        let bx: f32 = b.get("x")?;
        let by: f32 = b.get("y")?;
        let bz: f32 = b.get("z")?;
        let result = lua.create_table()?;
        result.set("x", ax + (bx - ax) * t)?;
        result.set("y", ay + (by - ay) * t)?;
        result.set("z", az + (bz - az) * t)?;
        Ok(result)
    })?)?;

    // Vec3.distance(a, b)
    vec3_mt.set("distance", lua.create_function(|_, (a, b): (Table, Table)| {
        let dx = a.get::<f32>("x")? - b.get::<f32>("x")?;
        let dy = a.get::<f32>("y")? - b.get::<f32>("y")?;
        let dz = a.get::<f32>("z")? - b.get::<f32>("z")?;
        Ok((dx * dx + dy * dy + dz * dz).sqrt())
    })?)?;

    skope.set("Vec3", vec3_mt)?;
    Ok(())
}

/// Quat (Quaternion) API
pub fn register_quat(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let quat_t = lua.create_table()?;

    // Quat.identity() - 단위 쿼터니언
    quat_t.set("identity", lua.create_function(|lua, ()| {
        let q = lua.create_table()?;
        q.set("x", 0.0)?;
        q.set("y", 0.0)?;
        q.set("z", 0.0)?;
        q.set("w", 1.0)?;
        Ok(q)
    })?)?;

    // Quat.from_axis_angle(axis, angle) - 축-각도로 쿼터니언 생성
    quat_t.set("from_axis_angle", lua.create_function(|lua, (axis, angle): (Table, f32)| {
        let ax: f32 = axis.get("x").unwrap_or(0.0);
        let ay: f32 = axis.get("y").unwrap_or(1.0);
        let az: f32 = axis.get("z").unwrap_or(0.0);

        // 축 정규화
        let len = (ax * ax + ay * ay + az * az).sqrt();
        let (ax, ay, az) = if len > 0.0001 {
            (ax / len, ay / len, az / len)
        } else {
            (0.0, 1.0, 0.0)
        };

        let half_angle = angle * 0.5;
        let s = half_angle.sin();
        let c = half_angle.cos();

        let q = lua.create_table()?;
        q.set("x", ax * s)?;
        q.set("y", ay * s)?;
        q.set("z", az * s)?;
        q.set("w", c)?;
        Ok(q)
    })?)?;

    // Quat.from_euler(x, y, z) - 오일러 각도로 쿼터니언 생성 (라디안)
    quat_t.set("from_euler", lua.create_function(|lua, (x, y, z): (f32, f32, f32)| {
        let (sx, cx) = (x * 0.5).sin_cos();
        let (sy, cy) = (y * 0.5).sin_cos();
        let (sz, cz) = (z * 0.5).sin_cos();

        let q = lua.create_table()?;
        q.set("x", sx * cy * cz - cx * sy * sz)?;
        q.set("y", cx * sy * cz + sx * cy * sz)?;
        q.set("z", cx * cy * sz - sx * sy * cz)?;
        q.set("w", cx * cy * cz + sx * sy * sz)?;
        Ok(q)
    })?)?;

    // Quat.from_euler_deg(x, y, z) - 오일러 각도로 쿼터니언 생성 (도)
    quat_t.set("from_euler_deg", lua.create_function(|lua, (x, y, z): (f32, f32, f32)| {
        let x = x.to_radians();
        let y = y.to_radians();
        let z = z.to_radians();

        let (sx, cx) = (x * 0.5).sin_cos();
        let (sy, cy) = (y * 0.5).sin_cos();
        let (sz, cz) = (z * 0.5).sin_cos();

        let q = lua.create_table()?;
        q.set("x", sx * cy * cz - cx * sy * sz)?;
        q.set("y", cx * sy * cz + sx * cy * sz)?;
        q.set("z", cx * cy * sz - sx * sy * cz)?;
        q.set("w", cx * cy * cz + sx * sy * sz)?;
        Ok(q)
    })?)?;

    // Quat.mul(a, b) - 쿼터니언 곱셈
    quat_t.set("mul", lua.create_function(|lua, (a, b): (Table, Table)| {
        let ax: f32 = a.get("x")?;
        let ay: f32 = a.get("y")?;
        let az: f32 = a.get("z")?;
        let aw: f32 = a.get("w")?;
        let bx: f32 = b.get("x")?;
        let by: f32 = b.get("y")?;
        let bz: f32 = b.get("z")?;
        let bw: f32 = b.get("w")?;

        let q = lua.create_table()?;
        q.set("x", aw * bx + ax * bw + ay * bz - az * by)?;
        q.set("y", aw * by - ax * bz + ay * bw + az * bx)?;
        q.set("z", aw * bz + ax * by - ay * bx + az * bw)?;
        q.set("w", aw * bw - ax * bx - ay * by - az * bz)?;
        Ok(q)
    })?)?;

    // Quat.normalize(q) - 쿼터니언 정규화
    quat_t.set("normalize", lua.create_function(|lua, q: Table| {
        let x: f32 = q.get("x")?;
        let y: f32 = q.get("y")?;
        let z: f32 = q.get("z")?;
        let w: f32 = q.get("w")?;

        let len = (x * x + y * y + z * z + w * w).sqrt();
        let result = lua.create_table()?;
        if len > 0.0001 {
            result.set("x", x / len)?;
            result.set("y", y / len)?;
            result.set("z", z / len)?;
            result.set("w", w / len)?;
        } else {
            result.set("x", 0.0)?;
            result.set("y", 0.0)?;
            result.set("z", 0.0)?;
            result.set("w", 1.0)?;
        }
        Ok(result)
    })?)?;

    // Quat.inverse(q) - 쿼터니언 역원
    quat_t.set("inverse", lua.create_function(|lua, q: Table| {
        let x: f32 = q.get("x")?;
        let y: f32 = q.get("y")?;
        let z: f32 = q.get("z")?;
        let w: f32 = q.get("w")?;

        let len_sq = x * x + y * y + z * z + w * w;
        let result = lua.create_table()?;
        if len_sq > 0.0001 {
            result.set("x", -x / len_sq)?;
            result.set("y", -y / len_sq)?;
            result.set("z", -z / len_sq)?;
            result.set("w", w / len_sq)?;
        } else {
            result.set("x", 0.0)?;
            result.set("y", 0.0)?;
            result.set("z", 0.0)?;
            result.set("w", 1.0)?;
        }
        Ok(result)
    })?)?;

    // Quat.slerp(a, b, t) - 구면 선형 보간
    quat_t.set("slerp", lua.create_function(|lua, (a, b, t): (Table, Table, f32)| {
        let ax: f32 = a.get("x")?;
        let ay: f32 = a.get("y")?;
        let az: f32 = a.get("z")?;
        let aw: f32 = a.get("w")?;
        let mut bx: f32 = b.get("x")?;
        let mut by: f32 = b.get("y")?;
        let mut bz: f32 = b.get("z")?;
        let mut bw: f32 = b.get("w")?;

        // 내적 계산
        let mut dot = ax * bx + ay * by + az * bz + aw * bw;

        // 최단 경로를 위해 필요시 부호 반전
        if dot < 0.0 {
            bx = -bx;
            by = -by;
            bz = -bz;
            bw = -bw;
            dot = -dot;
        }

        let result = lua.create_table()?;
        if dot > 0.9995 {
            // 거의 같은 경우 선형 보간
            result.set("x", ax + t * (bx - ax))?;
            result.set("y", ay + t * (by - ay))?;
            result.set("z", az + t * (bz - az))?;
            result.set("w", aw + t * (bw - aw))?;
        } else {
            let theta = dot.acos();
            let sin_theta = theta.sin();
            let wa = ((1.0 - t) * theta).sin() / sin_theta;
            let wb = (t * theta).sin() / sin_theta;

            result.set("x", wa * ax + wb * bx)?;
            result.set("y", wa * ay + wb * by)?;
            result.set("z", wa * az + wb * bz)?;
            result.set("w", wa * aw + wb * bw)?;
        }
        Ok(result)
    })?)?;

    // Quat.rotate_vec3(q, v) - 쿼터니언으로 벡터 회전
    quat_t.set("rotate_vec3", lua.create_function(|lua, (q, v): (Table, Table)| {
        let qx: f32 = q.get("x")?;
        let qy: f32 = q.get("y")?;
        let qz: f32 = q.get("z")?;
        let qw: f32 = q.get("w")?;
        let vx: f32 = v.get("x")?;
        let vy: f32 = v.get("y")?;
        let vz: f32 = v.get("z")?;

        // q * v * q^-1 계산 (최적화된 버전)
        let tx = 2.0 * (qy * vz - qz * vy);
        let ty = 2.0 * (qz * vx - qx * vz);
        let tz = 2.0 * (qx * vy - qy * vx);

        let result = lua.create_table()?;
        result.set("x", vx + qw * tx + qy * tz - qz * ty)?;
        result.set("y", vy + qw * ty + qz * tx - qx * tz)?;
        result.set("z", vz + qw * tz + qx * ty - qy * tx)?;
        Ok(result)
    })?)?;

    // Quat.look_at(forward, up) - 방향을 바라보는 쿼터니언 생성
    quat_t.set("look_at", lua.create_function(|lua, (forward, up): (Table, Table)| {
        let fx: f32 = forward.get("x")?;
        let fy: f32 = forward.get("y")?;
        let fz: f32 = forward.get("z")?;
        let ux: f32 = up.get("x").unwrap_or(0.0);
        let uy: f32 = up.get("y").unwrap_or(1.0);
        let uz: f32 = up.get("z").unwrap_or(0.0);

        // forward 정규화
        let f_len = (fx * fx + fy * fy + fz * fz).sqrt();
        let (fx, fy, fz) = if f_len > 0.0001 {
            (fx / f_len, fy / f_len, fz / f_len)
        } else {
            (0.0, 0.0, -1.0)
        };

        // right = up × forward
        let rx = uy * fz - uz * fy;
        let ry = uz * fx - ux * fz;
        let rz = ux * fy - uy * fx;
        let r_len = (rx * rx + ry * ry + rz * rz).sqrt();
        let (rx, ry, rz) = if r_len > 0.0001 {
            (rx / r_len, ry / r_len, rz / r_len)
        } else {
            (1.0, 0.0, 0.0)
        };

        // 실제 up = forward × right
        let ux = fy * rz - fz * ry;
        let uy = fz * rx - fx * rz;
        let uz = fx * ry - fy * rx;

        // 회전 행렬에서 쿼터니언 추출
        let trace = rx + uy + fz;
        let q = lua.create_table()?;

        if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            q.set("w", 0.25 * s)?;
            q.set("x", (uz - fy) / s)?;
            q.set("y", (fx - rz) / s)?;
            q.set("z", (ry - ux) / s)?;
        } else if rx > uy && rx > fz {
            let s = (1.0 + rx - uy - fz).sqrt() * 2.0;
            q.set("w", (uz - fy) / s)?;
            q.set("x", 0.25 * s)?;
            q.set("y", (ux + ry) / s)?;
            q.set("z", (fx + rz) / s)?;
        } else if uy > fz {
            let s = (1.0 + uy - rx - fz).sqrt() * 2.0;
            q.set("w", (fx - rz) / s)?;
            q.set("x", (ux + ry) / s)?;
            q.set("y", 0.25 * s)?;
            q.set("z", (fy + uz) / s)?;
        } else {
            let s = (1.0 + fz - rx - uy).sqrt() * 2.0;
            q.set("w", (ry - ux) / s)?;
            q.set("x", (fx + rz) / s)?;
            q.set("y", (fy + uz) / s)?;
            q.set("z", 0.25 * s)?;
        }

        Ok(q)
    })?)?;

    skope.set("Quat", quat_t)?;
    Ok(())
}

/// Math API
pub fn register_math(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let math_t = lua.create_table()?;

    // 상수
    math_t.set("PI", std::f32::consts::PI)?;
    math_t.set("TAU", std::f32::consts::TAU)?;
    math_t.set("E", std::f32::consts::E)?;

    // clamp
    math_t.set("clamp", lua.create_function(|_, (v, min, max): (f32, f32, f32)| {
        Ok(v.clamp(min, max))
    })?)?;

    // lerp
    math_t.set("lerp", lua.create_function(|_, (a, b, t): (f32, f32, f32)| {
        Ok(a + (b - a) * t)
    })?)?;

    // smoothstep
    math_t.set("smoothstep", lua.create_function(|_, (edge0, edge1, x): (f32, f32, f32)| {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        Ok(t * t * (3.0 - 2.0 * t))
    })?)?;

    // deg_to_rad
    math_t.set("deg_to_rad", lua.create_function(|_, deg: f32| {
        Ok(deg.to_radians())
    })?)?;

    // rad_to_deg
    math_t.set("rad_to_deg", lua.create_function(|_, rad: f32| {
        Ok(rad.to_degrees())
    })?)?;

    // sign
    math_t.set("sign", lua.create_function(|_, v: f32| {
        Ok(if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 })
    })?)?;

    skope.set("Math", math_t)?;
    Ok(())
}
