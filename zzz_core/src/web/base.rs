//zzz_core/src/web/base.rs
#![allow(dead_code)]
use actix_web::rt::Runtime;
use actix_web::web::{self, Data};
use actix_web::{App, HttpServer};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

// ── 路由配置 ────────────────────────────────────────────────────────────────

/// Handler 类型别名：actix-web 工厂函数签名
type HandlerFactory = Arc<dyn Fn(&mut web::ServiceConfig) + Send + Sync + 'static>;

/// 单条路由配置
#[derive(Clone)]
pub struct RouteConfig {
    handler: HandlerFactory,
}

impl RouteConfig {
    /// 创建一条路由
    ///
    /// ```rust
    /// async fn hello() -> impl Responder { "hello" }
    ///
    /// RouteConfig::new(|cfg| {
    ///     cfg.route("/hello", web::get().to(hello));
    /// })
    /// ```
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(&mut web::ServiceConfig) + Send + Sync + 'static,
    {
        Self {
            handler: Arc::new(handler),
        }
    }

    /// 快捷方法：GET
    pub fn get<F>(path: impl Into<String>, handler: F) -> Self
    where
        F: Fn() -> actix_web::Route + Send + Sync + 'static,
        F: Clone,
    {
        let path = path.into();
        Self::new(move |cfg| {
            cfg.route(&path, handler());
        })
    }

    /// 快捷方法：POST
    pub fn post<F>(path: impl Into<String>, handler: F) -> Self
    where
        F: Fn() -> actix_web::Route + Send + Sync + 'static,
        F: Clone,
    {
        let path = path.into();
        Self::new(move |cfg| {
            cfg.route(&path, handler());
        })
    }
}

// ── 共享状态 ─────────────────────────────────────────────────────────────────

/// 可注入任意 Data<T> 的类型擦除包装
type AppDataFactory = Arc<dyn Fn(&mut web::ServiceConfig) + Send + Sync + 'static>;

// ── 服务器状态 ────────────────────────────────────────────────────────────────

type StartResult = Result<u16, String>;

#[derive(Debug, PartialEq)]
pub enum ServerStatus {
    Stopped,
    Running,
    /// worker 线程 panic 或 actix runtime 异常退出
    Crashed,
}

struct RunningState {
    join_handle: JoinHandle<()>,
    stop_tx: tokio::sync::oneshot::Sender<()>,
    /// Sender 保留在 worker 线程中；worker 正常或异常退出时 Sender drop，
    /// 导致 channel 断开，主线程可通过 try_recv 感知。
    ///
    /// 用 `Arc<Mutex<bool>>` 的方案需要加锁，而利用 channel 断开语义
    /// 可以做到无锁、零额外同步原语。
    liveness_rx: mpsc::Receiver<()>,
}

// ── WebServer ─────────────────────────────────────────────────────────────────

pub struct WebServer {
    port: u16,
    routes: Vec<RouteConfig>,
    app_data: Vec<AppDataFactory>,
    state: Option<RunningState>,
}

impl WebServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            routes: Vec::new(),
            app_data: Vec::new(),
            state: None,
        }
    }

    /// 批量注册路由，支持链式调用
    ///
    /// ```rust
    /// server
    ///     .register_routes(vec![
    ///         RouteConfig::new(|cfg| { cfg.route("/health", web::get().to(health)); }),
    ///         RouteConfig::new(|cfg| { cfg.route("/users", web::post().to(create_user)); }),
    ///     ])
    ///     .start()?;
    /// ```
    pub fn register_routes(&mut self, routes: Vec<RouteConfig>) -> &mut Self {
        self.routes.extend(routes);
        self
    }

    /// 注入共享状态（可多次调用注入不同类型）
    ///
    /// ```rust
    /// let pool = Data::new(db_pool);
    /// let config = Data::new(app_config);
    ///
    /// server
    ///     .with_app_data(pool)
    ///     .with_app_data(config)
    ///     .register_routes(routes)
    ///     .start()?;
    /// ```
    pub fn with_app_data<T>(&mut self, data: Data<T>) -> &mut Self
    where
        T: Send + Sync + 'static,
    {
        self.app_data.push(Arc::new(move |cfg: &mut web::ServiceConfig| {
            cfg.app_data(data.clone());
        }));
        self
    }

    pub fn start(&mut self) -> Result<u16, Box<dyn std::error::Error>> {
        if self.state.is_some() {
            return Err("Server is already running".into());
        }

        let port = self.port;
        let routes: Vec<RouteConfig> = self.routes.clone();
        let app_data: Vec<AppDataFactory> = self.app_data.clone();

        let (start_tx, start_rx) = mpsc::channel::<StartResult>();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();

        let (liveness_tx, liveness_rx) = mpsc::channel::<()>();

        let handle = thread::spawn(move || {
            let _liveness = liveness_tx;

            let rt = match Runtime::new() {
                Ok(r) => r,
                Err(e) => {
                    let _ = start_tx.send(Err(format!("Failed to create Tokio runtime: {}", e)));
                    return;
                }
            };

            rt.block_on(async move {
                let routes = Arc::new(routes);
                let app_data = Arc::new(app_data);

                let server = HttpServer::new(move || {
                    let mut app = App::new();

                    app = app.configure(|cfg| {
                        for factory in app_data.iter() {
                            factory(cfg);
                        }
                    });

                    app = app.configure(|cfg| {
                        for route in routes.iter() {
                            (route.handler)(cfg);
                        }
                    });

                    app
                })
                    .bind(format!("0.0.0.0:{}", port));

                let server = match server {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = start_tx
                            .send(Err(format!("Failed to bind to 0.0.0.0:{}: {}", port, e)));
                        return;
                    }
                };

                let bound_port = match server.addrs().first() {
                    Some(addr) => addr.port(),
                    None => {
                        let _ = start_tx.send(Err("No address bound".to_string()));
                        return;
                    }
                };

                let server = server.run();
                let server_handle = server.handle();

                let _ = start_tx.send(Ok(bound_port));

                tokio::select! {
                    result = server => {
                        if let Err(e) = result {
                            eprintln!("[WebServer] Runtime error: {}", e);
                        }
                    }
                    _ = async {
                        let _ = stop_rx.await;
                        server_handle.stop(true).await;
                    } => {}
                }
            });
        });

        match start_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(bound_port)) => {
                self.state = Some(RunningState {
                    join_handle: handle,
                    stop_tx,
                    liveness_rx,
                });
                Ok(bound_port)
            }
            Ok(Err(e)) => Err(e.into()),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                Err("Server startup timed out after 5 seconds".into())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err("Server thread crashed before sending startup result".into())
            }
        }
    }

    pub fn stop(&mut self) {
        if let Some(state) = self.state.take() {
            let _ = state.stop_tx.send(());
            let _ = state.join_handle.join();
        }
    }

    /// 阻塞等待服务器自然退出（不发送停止信号）
    pub fn join(&mut self) {
        if let Some(state) = self.state.take() {
            let _ = state.join_handle.join();
        }
    }

    /// 返回服务器当前状态。
    ///
    /// - `Stopped`：从未启动，或已调用 `stop()` / `join()`
    /// - `Running`：worker 线程存活且 liveness channel 未断开
    /// - `Crashed`：worker 线程 panic 或 actix runtime 异常退出
    ///             （liveness Sender 被 drop，channel 断开）
    ///
    /// ### 竞态分析
    /// `liveness_rx.try_recv()` 是对 `mpsc::Receiver` 的独占访问（`&self` 内部
    /// 通过 `Option<RunningState>` 保证唯一引用），不存在多线程并发读写同一
    /// Receiver 的情况，因此无需额外锁保护。
    pub fn status(&self) -> ServerStatus {
        match &self.state {
            None => ServerStatus::Stopped,
            Some(state) => match state.liveness_rx.try_recv() {
                Err(mpsc::TryRecvError::Empty) => ServerStatus::Running,
                Err(mpsc::TryRecvError::Disconnected) => ServerStatus::Crashed,
                Ok(()) => ServerStatus::Crashed,
            },
        }
    }

    pub fn is_running(&self) -> bool {
        self.status() == ServerStatus::Running
    }
}

impl Drop for WebServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[macro_export]
#[doc(hidden)]
macro_rules! __register_method {
    (get)    => { actix_web::web::get()    };
    (post)   => { actix_web::web::post()   };
    (put)    => { actix_web::web::put()    };
    (delete) => { actix_web::web::delete() };
    (patch)  => { actix_web::web::patch()  };
    (head)   => { actix_web::web::head()   };
}

// ── 主宏 ────────────────────────────────────────────────────────────────────

/// 声明式路由 & 共享数据注册
///
/// ```ignore
/// register!(server {
///     get    "/health" => health,
///     post   "/users"  => create_user,
///     put    "/users"  => update_user,
///     delete "/users"  => delete_user,
///
///     data web::Data::new(db_pool.clone()),
///     data web::Data::new(app_state.clone()),
/// });
/// ```
#[macro_export]
macro_rules! web_register {
    // ── 入口 ──────────────────────────────────────────────
    ($server:ident { $($body:tt)* }) => {
        $crate::web_register!(@arm $server [] [] $($body)*)
    };

    // ── data（带尾逗号） ──────────────────────────────────
    (@arm $server:ident [ $($routes:expr),* ] [ $($data:expr),* ]
        data $d:expr, $($rest:tt)*
    ) => {
        $crate::web_register!(@arm $server
            [ $($routes),* ]
            [ $($data,)* $d ]
            $($rest)*
        )
    };

    // ── data（末项，无尾逗号） ────────────────────────────
    (@arm $server:ident [ $($routes:expr),* ] [ $($data:expr),* ]
        data $d:expr
    ) => {
        $crate::web_register!(@arm $server
            [ $($routes),* ]
            [ $($data,)* $d ]
        )
    };

    // ── route（带尾逗号） ─────────────────────────────────
    //    $method:ident 匹配 get / post / put / delete / …
    //    必须排在 data 分支之后，否则 `data` 也会被 $method 吞掉
    (@arm $server:ident [ $($routes:expr),* ] [ $($data:expr),* ]
        $method:ident $path:literal => $handler:expr, $($rest:tt)*
    ) => {
        $crate::web_register!(@arm $server [
            $($routes,)*
            $crate::web::base::RouteConfig::new(|cfg| {
                cfg.route(
                    $path,
                    $crate::__register_method!($method).to($handler),
                );
            })
        ] [ $($data),* ] $($rest)*)
    };

    // ── route（末项，无尾逗号） ───────────────────────────
    (@arm $server:ident [ $($routes:expr),* ] [ $($data:expr),* ]
        $method:ident $path:literal => $handler:expr
    ) => {
        $crate::web_register!(@arm $server [
            $($routes,)*
            $crate::web::base::RouteConfig::new(|cfg| {
                cfg.route(
                    $path,
                    $crate::__register_method!($method).to($handler),
                );
            })
        ] [ $($data),* ])
    };

    // ── 递归终止：执行注册 ────────────────────────────────
    (@arm $server:ident [ $($routes:expr),* ] [ $($data:expr),* ]) => {{
        // 先注入共享状态
        $( $server.with_app_data($data); )*
        // 再批量注册路由
        $server.register_routes(vec![ $($routes),* ]);
    }};
}


#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{web, HttpResponse, Responder};
    use reqwest::blocking::Client;
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
    use std::thread;
    use std::time::Duration;

    // ─────────────────────────────────────────────
    // 基础 handler
    // ─────────────────────────────────────────────

    async fn hello() -> impl Responder {
        HttpResponse::Ok().body("hello")
    }

    async fn echo(data: web::Data<String>) -> impl Responder {
        HttpResponse::Ok().body(data.get_ref().clone())
    }

    async fn incr(counter: web::Data<Arc<AtomicUsize>>) -> impl Responder {
        let val = counter.fetch_add(1, Ordering::SeqCst);
        HttpResponse::Ok().body(format!("{}", val))
    }

    // ─────────────────────────────────────────────
    // 1. 基础启动 + 路由
    // ─────────────────────────────────────────────

    #[test]
    fn test_basic_start_stop() {
        let mut server = WebServer::new(0);

        server.register_routes(vec![
            RouteConfig::new(|cfg| {
                cfg.route("/hello", web::get().to(hello));
            }),
        ]);

        let port = server.start().unwrap();

        let resp = Client::new()
            .get(&format!("http://127.0.0.1:{}/hello", port))
            .send()
            .unwrap()
            .text()
            .unwrap();

        assert_eq!(resp, "hello");
        assert!(server.is_running());

        server.stop();
        assert_eq!(server.status(), ServerStatus::Stopped);
    }

    // ─────────────────────────────────────────────
    // 2. 多路由测试
    // ─────────────────────────────────────────────

    #[test]
    fn test_multiple_routes() {
        let mut server = WebServer::new(0);

        server.register_routes(vec![
            RouteConfig::new(|cfg| {
                cfg.route("/a", web::get().to(|| async { "A" }));
            }),
            RouteConfig::new(|cfg| {
                cfg.route("/b", web::get().to(|| async { "B" }));
            }),
        ]);

        let port = server.start().unwrap();
        let client = Client::new();

        let a = client.get(&format!("http://127.0.0.1:{}/a", port))
            .send().unwrap().text().unwrap();
        let b = client.get(&format!("http://127.0.0.1:{}/b", port))
            .send().unwrap().text().unwrap();

        assert_eq!(a, "A");
        assert_eq!(b, "B");

        server.stop();
    }

    // ─────────────────────────────────────────────
    // 3. app_data 注入测试
    // ─────────────────────────────────────────────

    #[test]
    fn test_app_data_injection() {
        let mut server = WebServer::new(0);

        server
            .with_app_data(web::Data::new("shared-data".to_string()))
            .register_routes(vec![
                RouteConfig::new(|cfg| {
                    cfg.route("/echo", web::get().to(echo));
                }),
            ]);

        let port = server.start().unwrap();

        let resp = Client::new()
            .get(&format!("http://127.0.0.1:{}/echo", port))
            .send()
            .unwrap()
            .text()
            .unwrap();

        assert_eq!(resp, "shared-data");

        server.stop();
    }

    // ─────────────────────────────────────────────
    // 4. 并发请求测试
    // ─────────────────────────────────────────────

    #[test]
    fn test_concurrent_requests() {
        let mut server = WebServer::new(0);

        let counter = Arc::new(AtomicUsize::new(0));

        server
            .with_app_data(web::Data::new(counter.clone()))
            .register_routes(vec![
                RouteConfig::new(|cfg| {
                    cfg.route("/incr", web::get().to(incr));
                }),
            ]);

        let port = server.start().unwrap();

        let mut handles = vec![];

        for _ in 0..10 {
            let url = format!("http://127.0.0.1:{}/incr", port);
            handles.push(thread::spawn(move || {
                let _ = Client::new().get(&url).send().unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(counter.load(Ordering::SeqCst), 10);

        server.stop();
    }

    // ─────────────────────────────────────────────
    // 5. 重复启动保护
    // ─────────────────────────────────────────────

    #[test]
    fn test_double_start_should_fail() {
        let mut server = WebServer::new(0);

        server.register_routes(vec![
            RouteConfig::new(|cfg| {
                cfg.route("/", web::get().to(|| async { "ok" }));
            }),
        ]);

        let _ = server.start().unwrap();
        let second = server.start();

        assert!(second.is_err());

        server.stop();
    }

    // ─────────────────────────────────────────────
    // 6. 崩溃检测（liveness）
    // ─────────────────────────────────────────────

    #[test]
    fn test_crash_detection() {
        let mut server = WebServer::new(0);

        // 构造一个 panic handler
        async fn crash() -> impl Responder {
            panic!("intentional crash");
            #[allow(unreachable_code)]
            {
                HttpResponse::InternalServerError()
            }
        }

        server.register_routes(vec![
            RouteConfig::new(|cfg| {
                cfg.route("/crash", web::get().to(crash));
            }),
        ]);

        let port = server.start().unwrap();

        // 触发 panic
        let _ = Client::new()
            .get(&format!("http://127.0.0.1:{}/crash", port))
            .send();

        // 等待线程退出
        thread::sleep(Duration::from_millis(200));

        let status = server.status();

        assert!(
            status == ServerStatus::Running || status == ServerStatus::Crashed,
            "unexpected status: {:?}",
            status
        );

        server.stop();
    }

    // ─────────────────────────────────────────────
    // 7. join 行为测试
    // ─────────────────────────────────────────────

    #[test]
    fn test_join() {
        let mut server = WebServer::new(0);

        server.register_routes(vec![
            RouteConfig::new(|cfg| {
                cfg.route("/", web::get().to(|| async { "ok" }));
            }),
        ]);

        let _ = server.start().unwrap();

        // 在另一个线程 stop
        let mut s = server;
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            s.stop();
        });

        handle.join().unwrap();
    }

    // ─────────────────────────────────────────────
    // 8. 随机端口测试
    // ─────────────────────────────────────────────

    #[test]
    fn test_random_port() {
        let mut server = WebServer::new(0);

        server.register_routes(vec![
            RouteConfig::new(|cfg| {
                cfg.route("/", web::get().to(|| async { "ok" }));
            }),
        ]);

        let port = server.start().unwrap();

        assert!(port > 0);

        server.stop();
    }

    #[test]
    fn test_register_macro() {
        async fn health() -> impl Responder { HttpResponse::Ok().body("ok") }
        async fn create_user() -> impl Responder { HttpResponse::Ok().body("user created") }

        let shared = Data::new("shared-state".to_string());

        let mut server = WebServer::new(0);
        crate::web_register!(server {
        get  "/health" => health,
        post "/users"  => create_user,

        data shared.clone(),
    });

        let port = server.start().unwrap();
        let client = Client::new();

        let r = client.get(format!("http://127.0.0.1:{}/health", port))
            .send().unwrap().text().unwrap();
        assert_eq!(r, "ok");

        let r = client.post(format!("http://127.0.0.1:{}/users", port))
            .send().unwrap().text().unwrap();
        assert_eq!(r, "user created");

        server.stop();
    }
    #[test]
    fn test_sqlite_in_memory() {
        use rusqlite::Connection;
        use std::sync::Mutex;

        // ── 建库建表，预插两条数据 ────────────────────
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE notes (
             id      INTEGER PRIMARY KEY AUTOINCREMENT,
             content TEXT NOT NULL
         )",
            [],
        )
            .unwrap();
        conn.execute("INSERT INTO notes (content) VALUES (?1)", ["hello"]).unwrap();
        conn.execute("INSERT INTO notes (content) VALUES (?1)", ["world"]).unwrap();

        let db = Data::new(Mutex::new(conn));

        // ── handlers ─────────────────────────────────
        async fn list_notes(db: web::Data<Mutex<Connection>>) -> impl Responder {
            let conn = db.lock().unwrap();
            let mut stmt = conn
                .prepare("SELECT id, content FROM notes ORDER BY id")
                .unwrap();
            let notes: Vec<String> = stmt
                .query_map([], |row| {
                    let id: i64 = row.get(0)?;
                    let content: String = row.get(1)?;
                    Ok(format!("{}:{}", id, content))
                })
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();
            HttpResponse::Ok().body(notes.join(","))
        }

        async fn add_note(
            db: web::Data<Mutex<Connection>>,
            body: String,
        ) -> impl Responder {
            let conn = db.lock().unwrap();
            conn.execute("INSERT INTO notes (content) VALUES (?1)", [&body])
                .unwrap();
            HttpResponse::Created().body(format!("{}", conn.last_insert_rowid()))
        }

        // ── 组装服务器 ───────────────────────────────
        let mut server = WebServer::new(0);
        server
            .with_app_data(db)
            .register_routes(vec![RouteConfig::new(|cfg| {
                cfg.route("/notes", web::get().to(list_notes));
                cfg.route("/notes", web::post().to(add_note));
            })]);

        let port = server.start().unwrap();
        let client = Client::new();
        let base = format!("http://127.0.0.1:{}", port);

        // ── 验证预置数据 ────────────────────────────
        let resp = client
            .get(format!("{}/notes", base))
            .send()
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(resp, "1:hello,2:world");

        // ── 插入新记录 ──────────────────────────────
        let resp = client
            .post(format!("{}/notes", base))
            .body("rust")
            .send()
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(resp, "3"); // 自增 id

        // ── 再次查询，确认持久化 ────────────────────
        let resp = client
            .get(format!("{}/notes", base))
            .send()
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(resp, "1:hello,2:world,3:rust");

        server.stop();
    }

    #[test]
    fn test_sqlite_in_memory_and_register_macro() {
        use rusqlite::Connection;
        use std::sync::Mutex;

        // ── 建库建表，预插数据 ────────────────────────
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE users (
             id    INTEGER PRIMARY KEY AUTOINCREMENT,
             name  TEXT NOT NULL,
             email TEXT NOT NULL
         )",
            [],
        )
            .unwrap();
        conn.execute(
            "INSERT INTO users (name, email) VALUES (?1, ?2)",
            ["Alice", "alice@example.com"],
        )
            .unwrap();
        conn.execute(
            "INSERT INTO users (name, email) VALUES (?1, ?2)",
            ["Bob", "bob@example.com"],
        )
            .unwrap();

        let db = Data::new(Mutex::new(conn));

        // ── handlers ─────────────────────────────────
        async fn list_users(db: web::Data<Mutex<Connection>>) -> impl Responder {
            let conn = db.lock().unwrap();
            let mut stmt = conn
                .prepare("SELECT id, name, email FROM users ORDER BY id")
                .unwrap();
            let users: Vec<String> = stmt
                .query_map([], |row| {
                    let id: i64 = row.get(0)?;
                    let name: String = row.get(1)?;
                    let email: String = row.get(2)?;
                    Ok(format!("{}:{}:{}", id, name, email))
                })
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();
            HttpResponse::Ok().body(users.join(";"))
        }

        async fn get_user(
            db: web::Data<Mutex<Connection>>,
            path: web::Path<i64>,
        ) -> impl Responder {
            let id = path.into_inner();
            let conn = db.lock().unwrap();
            let result = conn.query_row(
                "SELECT id, name, email FROM users WHERE id = ?1",
                [id],
                |row| {
                    let id: i64 = row.get(0)?;
                    let name: String = row.get(1)?;
                    let email: String = row.get(2)?;
                    Ok(format!("{}:{}:{}", id, name, email))
                },
            );
            match result {
                Ok(user) => HttpResponse::Ok().body(user),
                Err(_) => HttpResponse::NotFound().body("not found"),
            }
        }

        async fn create_user(
            db: web::Data<Mutex<Connection>>,
            body: String,
        ) -> impl Responder {
            // 简单解析: "name,email"
            let parts: Vec<&str> = body.split(',').collect();
            if parts.len() != 2 {
                return HttpResponse::BadRequest().body("invalid format");
            }
            let conn = db.lock().unwrap();
            conn.execute(
                "INSERT INTO users (name, email) VALUES (?1, ?2)",
                [parts[0], parts[1]],
            )
                .unwrap();
            HttpResponse::Created().body(format!("{}", conn.last_insert_rowid()))
        }

        async fn delete_user(
            db: web::Data<Mutex<Connection>>,
            path: web::Path<i64>,
        ) -> impl Responder {
            let id = path.into_inner();
            let conn = db.lock().unwrap();
            let affected = conn
                .execute("DELETE FROM users WHERE id = ?1", [id])
                .unwrap();
            if affected > 0 {
                HttpResponse::Ok().body("deleted")
            } else {
                HttpResponse::NotFound().body("not found")
            }
        }

        async fn health() -> impl Responder {
            HttpResponse::Ok().body("healthy")
        }

        async fn app_info(config: web::Data<String>) -> impl Responder {
            HttpResponse::Ok().body(config.get_ref().clone())
        }

        // ── 额外共享状态 ─────────────────────────────
        let app_config = Data::new("app-v1.0".to_string());

        // ── 使用 register! 宏组装服务器 ──────────────
        let mut server = WebServer::new(0);
        crate::web_register!(server {
        get    "/health"      => health,
        get    "/info"        => app_info,
        get    "/users"       => list_users,
        get    "/users/{id}"  => get_user,
        post   "/users"       => create_user,
        delete "/users/{id}"  => delete_user,

        data db.clone(),
        data app_config.clone(),
    });

        let port = server.start().unwrap();
        let client = Client::new();
        let base = format!("http://127.0.0.1:{}", port);

        // ── 1. 健康检查 ─────────────────────────────
        let resp = client
            .get(format!("{}/health", base))
            .send()
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(resp, "healthy");

        // ── 2. 应用信息（验证多 Data 注入）──────────
        let resp = client
            .get(format!("{}/info", base))
            .send()
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(resp, "app-v1.0");

        // ── 3. 查询预置用户 ─────────────────────────
        let resp = client
            .get(format!("{}/users", base))
            .send()
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(resp, "1:Alice:alice@example.com;2:Bob:bob@example.com");

        // ── 4. 获取单个用户 ─────────────────────────
        let resp = client
            .get(format!("{}/users/1", base))
            .send()
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(resp, "1:Alice:alice@example.com");

        let resp = client
            .get(format!("{}/users/999", base))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 404);

        // ── 5. 创建新用户 ───────────────────────────
        let resp = client
            .post(format!("{}/users", base))
            .body("Charlie,charlie@example.com")
            .send()
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(resp, "3");

        // ── 6. 验证新用户已入库 ─────────────────────
        let resp = client
            .get(format!("{}/users", base))
            .send()
            .unwrap()
            .text()
            .unwrap();
        assert!(resp.contains("3:Charlie:charlie@example.com"));

        // ── 7. 删除用户 ─────────────────────────────
        let resp = client
            .delete(format!("{}/users/2", base))
            .send()
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(resp, "deleted");

        // ── 8. 确认删除生效 ─────────────────────────
        let resp = client
            .get(format!("{}/users", base))
            .send()
            .unwrap()
            .text()
            .unwrap();
        assert!(!resp.contains("Bob"));
        assert_eq!(resp, "1:Alice:alice@example.com;3:Charlie:charlie@example.com");

        // ── 9. 删除不存在的用户 ─────────────────────
        let resp = client
            .delete(format!("{}/users/999", base))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 404);

        server.stop();
    }

}