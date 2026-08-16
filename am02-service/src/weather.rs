//! P6：天气采集模块。
//!
//! 逆向结论（AYASpaceCef.exe `CMiniPCLauncher::UpdateInfo` + 字符串表）：
//! 原版 AYASpace 用 **OpenWeatherMap** 拉天气，API key 硬编码在二进制里：
//!   `http://api.openweathermap.org/data/2.5/weather?lon={lon}&lat={lat}
//!        &appid=6a7a50163bdf8fde936988373a86ad40&lang=zh_cn&units=metric`
//! 城市搜索用 `geo/1.0/direct?q=`。
//! 响应字段：`weather[0].description`（天气）、`main.temp`（气温）、
//! `wind.speed`（风速 m/s）、`wind.deg`（风向角度）。
//!
//! 风速 m/s 换算蒲福风级（原版逐档比较，阈值同标准蒲福风级），
//! 风向角度换算 8 方位中文（北/东北/东/东南/南/西南/西/西北风）。
//!
//! 设计：后台线程周期拉取（默认 10 分钟）+ IPC 设地区后立即唤醒。
//! 拉取失败保持旧值/空，**绝不 panic、绝不阻塞主循环推帧**。
//! 零新增依赖：用 std::net::TcpStream 走明文 HTTP（与原版 http:// 一致）。

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::protocol::WeatherSnapshot;

/// OpenWeatherMap API key（从原版 AYASpaceCef.exe 逆向提取，免费档）。
pub const OWM_APPID: &str = "6a7a50163bdf8fde936988373a86ad40";

/// 天气拉取周期：天气变化慢，10 分钟足矣；IPC 设地区会立即唤醒。
const REFRESH_INTERVAL: Duration = Duration::from_secs(600);
/// HTTP 单次超时（连接/读写）。
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

/// location 持久化文件：am02-service 重启后恢复天气地区。
/// 否则重启后 location 丢失，插件 get_state 反向同步会把空值覆盖回本地，导致「城市被清空」。
const LOCATION_FILE: &str = "/var/lib/am02-service/location.conf";

/// 从持久化文件读回上次设置的天气地区；无文件/空文件返回 None。
pub fn load_location() -> Option<String> {
    let s = std::fs::read_to_string(LOCATION_FILE).ok()?;
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 持久化天气地区：Some(城市) 写文件，None 删文件（关闭天气）。
pub fn save_location(loc: &Option<String>) {
    match loc {
        Some(city) => {
            let _ = std::fs::write(LOCATION_FILE, city.as_bytes());
        }
        None => {
            let _ = std::fs::remove_file(LOCATION_FILE);
        }
    }
}

/// 共享天气状态：`location` 由 IPC 写（set_location），`snapshot` 由天气线程写，
/// 主循环每帧读 snapshot 后写 pack。两者不同时写，各自持锁即可。
pub struct WeatherState {
    pub location: Option<String>,
    pub snapshot: WeatherSnapshot,
}

impl WeatherState {
    pub fn new() -> Self {
        Self {
            location: load_location(),
            snapshot: WeatherSnapshot::default(),
        }
    }
}

/// 启动天气后台线程，返回「唤醒通道」：IPC 设地区后发 `()` 触发立即拉取。
pub fn spawn_weather_loop(state: Arc<Mutex<WeatherState>>, notify: Sender<()>) -> Sender<()> {
    let (wake_tx, wake_rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || weather_loop(state, notify, wake_rx));
    wake_tx
}

fn weather_loop(state: Arc<Mutex<WeatherState>>, notify: Sender<()>, wake_rx: Receiver<()>) {
    // 启动即拉一次（若已设地区），随后按周期/唤醒。
    fetch_and_publish(&state, &notify);
    loop {
        // 唤醒信号或到刷新周期，二者任一即拉取。
        let _ = wake_rx.recv_timeout(REFRESH_INTERVAL);
        fetch_and_publish(&state, &notify);
    }
}

/// 拉取一次天气并写回共享状态；成功后通知主循环立即回推一帧。
fn fetch_and_publish(state: &Arc<Mutex<WeatherState>>, notify: &Sender<()>) {
    let location = { state.lock().unwrap().location.clone() };
    let Some(loc) = location else {
        return; // 未设地区：不拉、不清（清理由 IPC set_location 处理）
    };
    match fetch_weather(&loc) {
        Some(snap) => {
            state.lock().unwrap().snapshot = snap;
            // 通知主循环立即回推（复用语言/主题/功耗那条 IPC 通道）
            let _ = notify.send(());
        }
        None => tracing::warn!("天气拉取失败（地区={loc}），保持旧值"),
    }
}

// ── OpenWeatherMap 响应 JSON ──

#[derive(Deserialize)]
struct OwmResponse {
    name: Option<String>,
    weather: Option<Vec<OwmWeather>>,
    main: Option<OwmMain>,
    wind: Option<OwmWind>,
}

#[derive(Deserialize)]
struct OwmWeather {
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize)]
struct OwmMain {
    #[serde(default)]
    temp: Option<f32>,
}

#[derive(Deserialize)]
struct OwmWind {
    #[serde(default)]
    speed: Option<f32>,
    #[serde(default)]
    deg: Option<f32>,
}

/// geo API 返回项（城市搜索结果）。
#[derive(Deserialize)]
struct GeoItem {
    lat: f64,
    lon: f64,
}

// ── 本地中文行政区划映射表 ──

/// 一个行政区划条目（省/市/县区/市辖区），由 `cities.json` 编译进二进制。
///
/// `aliases` 兼容常见写法（「深圳市」→「深圳」），`province`/`city` 提供
/// 三级区划关系（如「南山区」→ 广东省 / 深圳市），`prio` 用于同名区县
/// 消歧（直辖市 > 副省级 > 省会 > 普通）。
#[derive(Debug, Clone, Deserialize, Serialize)]
struct CityEntry {
    name: String,
    #[serde(default)]
    aliases: Vec<String>,
    lat: f64,
    lon: f64,
    province: String,
    #[serde(default)]
    city: String,
    #[serde(default)]
    prio: u8,
}

/// 城市搜索结果项（回写插件，供用户区分同名区县）。
#[derive(Debug, Clone, Serialize)]
pub struct CityHit {
    pub name: String,
    pub province: String,
    pub city: String,
    pub lat: f64,
    pub lon: f64,
}

/// 懒加载本地映射表（首次调用时解析一次，约 3250 条）。
static CITIES: OnceLock<Vec<CityEntry>> = OnceLock::new();

fn cities() -> &'static [CityEntry] {
    CITIES
        .get_or_init(|| serde_json::from_str(include_str!("cities.json")).unwrap_or_default())
        .as_slice()
}

/// 精确匹配本地表（标准名或别名相等），同名取 `prio` 最高者。
fn exact_match(query: &str) -> Option<&'static CityEntry> {
    let mut best: Option<&CityEntry> = None;
    for e in cities() {
        if e.name == query || e.aliases.iter().any(|a| a == query) {
            if best.map_or(true, |b| e.prio > b.prio) {
                best = Some(e);
            }
        }
    }
    best
}

/// 把 location 解析成经纬度：`lat,lon` 直接用，中文城市名先查本地表
/// （含「市/区」后缀各种写法），查不到回退 geo API（英文名/国外城市）。
/// （weather API 不接受中文城市名，直接 `q=深圳` 会 404 city not found。）
pub fn resolve_coords(location: &str) -> Option<(f64, f64)> {
    let loc = location.trim();
    if loc.is_empty() {
        return None;
    }
    // "lat,lon" 直接解析
    if let Some((lat_s, lon_s)) = loc.split_once(',') {
        if let (Ok(lat), Ok(lon)) = (lat_s.trim().parse::<f64>(), lon_s.trim().parse::<f64>()) {
            return Some((lat, lon));
        }
    }
    // 本地表精确匹配
    if let Some(e) = exact_match(loc) {
        return Some((e.lat, e.lon));
    }
    // 回退 geo API 搜经纬度（英文/国外城市）
    let q = urlencode(loc);
    let path = format!("/geo/1.0/direct?q={q}&limit=1&appid={OWM_APPID}");
    let body = http_get("api.openweathermap.org", &path).ok()?;
    let arr: Vec<GeoItem> = serde_json::from_str(&body).ok()?;
    let first = arr.into_iter().next()?;
    Some((first.lat, first.lon))
}

/// 拉取指定地区当前天气。`location` 可为城市名（中文/英文）或 `lat,lon`。
/// 返回 None 表示失败（网络/解析/无此地区），调用方保持旧值。
pub fn fetch_weather(location: &str) -> Option<WeatherSnapshot> {
    let (lat, lon) = resolve_coords(location)?;
    let path = format!(
        "/data/2.5/weather?lat={lat}&lon={lon}&appid={OWM_APPID}&lang=zh_cn&units=metric"
    );
    let body = http_get("api.openweathermap.org", &path).ok()?;
    let resp: OwmResponse = serde_json::from_str(&body).ok()?;

    // 错误响应（如 {"cod":401,...}）无 name/main 字段，判为失败
    if resp.name.is_none() && resp.main.is_none() {
        return None;
    }

    // 本地表精确命中时，副屏城市名用中文标准名 + 省份（对标原版中文显示）；
    // 未命中（英文/国外城市）则退回 OWM 返回名，省份留空。
    let matched = exact_match(location.trim());
    let city = matched
        .map(|e| e.name.clone())
        .or(resp.name)
        .unwrap_or_else(|| location.to_string());
    let province = matched.map(|e| e.province.clone()).unwrap_or_default();
    let weather = resp
        .weather
        .as_ref()
        .and_then(|w| w.first())
        .and_then(|w| w.description.clone())
        .unwrap_or_default();
    let temperature_c = resp.main.as_ref().and_then(|m| m.temp).unwrap_or(0.0);
    let (wind_direction, wind_power) = match &resp.wind {
        Some(w) => (
            wind_direction_zh(w.deg.unwrap_or(0.0)).to_string(),
            beaufort_level(w.speed.unwrap_or(0.0)),
        ),
        None => (String::new(), 0),
    };

    Some(WeatherSnapshot {
        city,
        weather,
        province,
        temperature_c,
        wind_direction,
        wind_power,
    })
}

/// 城市模糊搜索：对标准名 + 别名做包含匹配，返回去重候选（含省/地级市，
/// 供区分同名区县），按匹配度 → 行政地位排序，最多 10 条。
pub fn search_cities(query: &str) -> Vec<CityHit> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(i32, u8, &CityEntry)> = Vec::new();
    for e in cities() {
        let mut score = match_score(&e.name, q);
        for a in &e.aliases {
            score = score.max(match_score(a, q));
        }
        if score > 0 {
            scored.push((score, e.prio, e));
        }
    }
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then(b.1.cmp(&a.1))
            .then(a.2.name.cmp(&b.2.name))
    });
    scored.truncate(10);
    scored
        .into_iter()
        .map(|(_, _, e)| CityHit {
            name: e.name.clone(),
            province: e.province.clone(),
            city: e.city.clone(),
            lat: e.lat,
            lon: e.lon,
        })
        .collect()
}

/// 匹配打分：相等 3 > 前缀 2 > 包含 1 > 不匹配 0。
fn match_score(hay: &str, needle: &str) -> i32 {
    if hay == needle {
        3
    } else if hay.starts_with(needle) {
        2
    } else if hay.contains(needle) {
        1
    } else {
        0
    }
}

/// 风向角度 → 8 方位中文（45° 扇区，0=N，顺时针）。
fn wind_direction_zh(deg: f32) -> &'static str {
    const DIRS: [&str; 8] = [
        "北风", "东北风", "东风", "东南风", "南风", "西南风", "西风", "西北风",
    ];
    let d = deg.rem_euclid(360.0);
    let idx = ((d + 22.5) / 45.0) as usize % 8;
    DIRS[idx]
}

/// 风速 m/s → 蒲福风级（标准中国气象蒲福风级阈值，同原版逐档比较）。
fn beaufort_level(mps: f32) -> u8 {
    const THRESH: [f32; 12] = [
        0.3, 1.6, 3.4, 5.5, 8.0, 10.8, 13.9, 17.2, 20.8, 24.5, 28.5, 32.7,
    ];
    for (i, &t) in THRESH.iter().enumerate() {
        if mps < t {
            return i as u8;
        }
    }
    12
}

// ── 零依赖明文 HTTP GET ──

fn http_get(host: &str, path: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect((host, 80)).map_err(|e| format!("连接失败: {e}"))?;
    stream
        .set_read_timeout(Some(HTTP_TIMEOUT))
        .map_err(|e| format!("设读超时失败: {e}"))?;
    stream
        .set_write_timeout(Some(HTTP_TIMEOUT))
        .map_err(|e| format!("设写超时失败: {e}"))?;

    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: am02-service/0.1\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("写请求失败: {e}"))?;

    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .map_err(|e| format!("读响应失败: {e}"))?;
    let resp = String::from_utf8_lossy(&buf);
    let body = resp
        .split("\r\n\r\n")
        .nth(1)
        .ok_or_else(|| "响应缺少 body".to_string())?;
    Ok(body.to_string())
}

/// 极简 URL 编码（保留 RFC3986 非保留字符，空格 → `+`）。
fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wind_direction() {
        assert_eq!(wind_direction_zh(0.0), "北风");
        assert_eq!(wind_direction_zh(90.0), "东风");
        assert_eq!(wind_direction_zh(180.0), "南风");
        assert_eq!(wind_direction_zh(270.0), "西风");
        assert_eq!(wind_direction_zh(45.0), "东北风");
        assert_eq!(wind_direction_zh(360.0), "北风");
    }

    #[test]
    fn test_beaufort() {
        assert_eq!(beaufort_level(0.0), 0);
        assert_eq!(beaufort_level(0.5), 1);
        assert_eq!(beaufort_level(2.0), 2);
        assert_eq!(beaufort_level(4.0), 3);
        assert_eq!(beaufort_level(6.0), 4);
        assert_eq!(beaufort_level(20.0), 8);
        assert_eq!(beaufort_level(40.0), 12);
    }

    #[test]
    fn test_urlencode() {
        assert_eq!(urlencode("深圳"), "%E6%B7%B1%E5%9C%B3");
        assert_eq!(urlencode("New York"), "New+York");
        assert_eq!(urlencode("a-b.c~d"), "a-b.c~d");
    }

    #[test]
    fn test_resolve_local_cities() {
        // 深圳 / 深圳市 都命中深圳市（广东省）
        let (lat, lon) = resolve_coords("深圳").unwrap();
        assert!((lat - 22.547).abs() < 0.5, "深圳 lat={lat}");
        assert!((lon - 114.086).abs() < 0.5, "深圳 lon={lon}");
        assert_eq!(resolve_coords("深圳市").unwrap(), resolve_coords("深圳").unwrap());
        // 南山区：应命中深圳南山区（副省级 prio 90），非鹤岗（纬度 47）
        let (nlat, nlon) = resolve_coords("南山区").unwrap();
        assert!(nlat > 20.0 && nlat < 30.0, "南山区应命中深圳，got lat={nlat}");
        assert!((nlon - 113.93).abs() < 0.5, "南山区 lon={nlon}");
        // 别名「南山」同命中
        assert_eq!(resolve_coords("南山").unwrap(), resolve_coords("南山区").unwrap());
        // 北京
        let (blat, blon) = resolve_coords("北京").unwrap();
        assert!((blat - 39.905).abs() < 0.5);
        assert!((blon - 116.405).abs() < 0.5);
    }

    #[test]
    fn test_search_nanshan() {
        let hits = search_cities("南山");
        assert!(!hits.is_empty());
        // 至少含深圳南山区和鹤岗南山区，均带省份/地级市
        assert!(hits.iter().any(|h| h.name == "南山区" && h.province == "广东省" && h.city == "深圳市"));
        assert!(hits.iter().any(|h| h.name == "南山区" && h.province == "黑龙江省" && h.city == "鹤岗市"));
        assert!(hits.iter().all(|h| !h.province.is_empty()));
        assert!(hits.len() <= 10);
        // 深圳南山区（prio 90）应排在鹤岗（prio 50）之前
        let sz = hits.iter().position(|h| h.province == "广东省").unwrap();
        let hg = hits.iter().position(|h| h.province == "黑龙江省").unwrap();
        assert!(sz < hg);
    }

    #[test]
    fn test_search_shenzhen_alias() {
        let hits = search_cities("深圳市");
        assert!(hits.iter().any(|h| h.name == "深圳市" && h.province == "广东省"));
    }

}
