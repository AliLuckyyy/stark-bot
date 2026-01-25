# StarkBot Architecture Guidelines

## Authentication

### Session Token Protection

All API endpoints (except `/health` and `/api/auth/login`) MUST require session token authentication.

**How to protect an endpoint:**

1. Import the validation helper:
   ```rust
   use crate::controllers::api_keys::validate_session_from_request;
   ```

2. At the start of every handler, validate the session:
   ```rust
   pub async fn my_handler(
       state: web::Data<AppState>,
       req: HttpRequest,
   ) -> impl Responder {
       if let Err(resp) = validate_session_from_request(&state, &req) {
           return resp;
       }
       // ... handler logic
   }
   ```

3. The frontend must include the token in the Authorization header:
   ```javascript
   fetch('/api/endpoint', {
       headers: {
           'Authorization': `Bearer ${token}`
       }
   });
   ```

### Endpoints requiring auth:
- `GET /api/dashboard` - Dashboard data
- `GET /api/keys` - List API keys
- `POST /api/keys` - Add/update API key
- `DELETE /api/keys` - Remove API key
- `POST /api/chat` - Send message to AI agent

### Public endpoints:
- `GET /health` - Health check (for load balancers/DO App Platform)
- `POST /api/auth/login` - Login with secret key
- `POST /api/auth/logout` - Logout (invalidates token)
- `GET /api/auth/validate` - Check if token is valid

## Environment Variables

### Design Principle

**SECRET_KEY should be the ONLY required environment variable.**

All other API keys and secrets should be stored in the SQLite database and managed through the web UI.

### Required Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `SECRET_KEY` | Yes | The master secret for logging into StarkBot |
| `PORT` | No | Server port (default: 8080) |
| `DATABASE_URL` | No | SQLite database path (default: `./.db/stark.db`) |
| `RUST_LOG` | No | Log level (default: info) |

### Why this approach?

1. **Simpler deployment**: Only one secret to manage in cloud platform settings
2. **Better UX**: Users can add/remove API keys through the UI without redeploying
3. **More secure**: API keys are stored encrypted in the database, not in environment variables that might be logged
4. **Flexibility**: Easy to add new service integrations without code changes

## External API Keys

### Database Storage

External API keys are stored in the `external_api_keys` table:

```sql
CREATE TABLE IF NOT EXISTS external_api_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_name TEXT UNIQUE NOT NULL,
    api_key TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)
```

### Supported Services

| Service Name | Description | Used By |
|--------------|-------------|---------|
| `anthropic` | Anthropic/Claude API key | AI Agent chat |

### Adding a New Service

1. Add the service to the dropdown in `stark-frontend/api-keys.html`
2. In the backend controller that needs the key, fetch it from the database:
   ```rust
   let api_key = match state.db.get_api_key("service_name") {
       Ok(Some(key)) => key.api_key,
       Ok(None) => {
           return HttpResponse::ServiceUnavailable().json(ErrorResponse {
               success: false,
               error: Some("Service API key not configured.".to_string()),
           });
       }
       Err(e) => {
           log::error!("Database error: {}", e);
           return HttpResponse::InternalServerError().json(ErrorResponse {
               success: false,
               error: Some("Internal server error".to_string()),
           });
       }
   };
   ```

## Frontend Authentication Flow

1. User enters secret key on login page
2. Frontend sends POST to `/api/auth/login`
3. Backend validates against `SECRET_KEY` env var
4. If valid, creates session in SQLite and returns token
5. Frontend stores token in `localStorage` as `stark_token`
6. All subsequent API requests include `Authorization: Bearer <token>` header
7. On logout, frontend calls `/api/auth/logout` and clears localStorage
