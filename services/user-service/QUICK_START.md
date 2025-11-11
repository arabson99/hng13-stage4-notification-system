# Quick Start Guide - User Service

## 🚀 Prerequisites

- Python 3.11+
- PostgreSQL (running)
- Redis (running)
- Docker (optional, for containerized setup)

## 📦 Installation

### Option 1: Local Development

```bash
# Navigate to user-service directory
cd services/user-service

# Install dependencies
pip install -r requirements.txt

# Set environment variables (optional, defaults provided)
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/notification_db"
export REDIS_HOST="localhost"
export REDIS_PORT=6379

# Run the service
uvicorn app.main:app --host 0.0.0.0 --port 8001 --reload
```

### Option 2: Docker

```bash
# Build and run with docker-compose (from project root)
docker-compose up user-service
```

## ✅ Verify Service is Running

```bash
# Health check
curl http://localhost:8001/health

# Expected response:
# {
#   "status": "healthy",
#   "service": "user-service",
#   "database": "connected",
#   "redis": "connected"
# }
```

## 🧪 Quick Test (5 Steps)

### Step 1: Create a User

```bash
curl -X POST http://localhost:8001/users \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "email": "test@example.com",
    "password": "password123"
  }'
```

**Save the `id` from response!**

### Step 2: Login (Get Token)

```bash
curl -X POST http://localhost:8001/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "password": "password123"
  }'
```

**Copy the `access_token` from response!**

### Step 3: Get Your User Info

```bash
curl http://localhost:8001/users/me \
  -H "Authorization: Bearer YOUR_TOKEN_HERE"
```

### Step 4: Create Notification Preference

```bash
curl -X POST http://localhost:8001/users/1/preferences \
  -H "Authorization: Bearer YOUR_TOKEN_HERE" \
  -H "Content-Type: application/json" \
  -d '{
    "notification_type": "email",
    "channel": "marketing",
    "enabled": true
  }'
```

### Step 5: Get Preferences (Tests Redis Cache)

```bash
curl http://localhost:8001/users/1/preferences \
  -H "Authorization: Bearer YOUR_TOKEN_HERE"
```

## 📋 Essential Endpoints

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| `GET` | `/health` | ❌ | Health check |
| `POST` | `/users` | ❌ | Create user |
| `POST` | `/login` | ❌ | Login (get token) |
| `GET` | `/users/me` | ✅ | Get current user |
| `PUT` | `/users/{id}` | ✅ | Update user |
| `POST` | `/users/{id}/preferences` | ✅ | Create preference |
| `GET` | `/users/{id}/preferences` | ✅ | Get preferences (cached) |
| `PUT` | `/users/{id}/preferences/{pref_id}` | ✅ | Update preference |

## 🔑 Authentication

All endpoints marked with ✅ require authentication:

1. Login via `POST /login` to get a JWT token
2. Include token in requests: `Authorization: Bearer <token>`
3. Token expires in 30 minutes (default)

## 🎯 Postman Testing

### Import Collection

1. Open Postman
2. Click **Import**
3. Select `postman_collection.json` from this directory
4. Create environment with `base_url = http://localhost:8001`

### Quick Test Flow

1. ✅ **Health Check** → Verify service is up
2. ✅ **Create User** → Creates user and saves `user_id`
3. ✅ **Login** → Gets token and saves automatically
4. ✅ **Get Current User** → Tests authentication
5. ✅ **Create Preference** → Tests CRUD operations
6. ✅ **Get Preferences** → Tests Redis caching

## 🔧 Configuration

Environment variables (with defaults):

```bash
DATABASE_URL=postgresql://postgres:postgres@postgres:5432/notification_db
REDIS_HOST=redis
REDIS_PORT=6379
SECRET_KEY=your-secret-key-change-in-production
ACCESS_TOKEN_EXPIRE_MINUTES=30
```

## 📚 Full Documentation

- **Detailed Testing Guide**: See `POSTMAN_TESTING.md`
- **API Documentation**: Visit `http://localhost:8001/docs` (Swagger UI)
- **Alternative Docs**: Visit `http://localhost:8001/redoc` (ReDoc)

## 🐛 Troubleshooting

### "could not translate host name 'postgres' to address"
**Problem**: Running locally but config uses Docker service names.

**Solution**: 
1. The config now defaults to `localhost` - restart your service
2. Or create a `.env` file with:
   ```bash
   DATABASE_URL=postgresql://postgres:postgres@localhost:5432/notification_db
   REDIS_HOST=localhost
   ```
3. For Docker, set environment variables:
   ```bash
   DATABASE_URL=postgresql://postgres:postgres@postgres:5432/notification_db
   REDIS_HOST=redis
   ```

### Service won't start
- Check PostgreSQL is running: `pg_isready` (or `psql -U postgres`)
- Check Redis is running: `redis-cli ping`
- Verify port 8001 is available

### Database connection error
- **Local Development**: Ensure PostgreSQL is running on `localhost:5432`
- Verify `DATABASE_URL` is correct
- Check PostgreSQL credentials (username: `postgres`, password: `postgres`)
- Create database if it doesn't exist:
  ```bash
  psql -U postgres -c "CREATE DATABASE notification_db;"
  ```

### Redis connection error
- **Local Development**: Ensure Redis is running on `localhost:6379`
- Verify Redis is running: `redis-cli ping`
- Check `REDIS_HOST` and `REDIS_PORT` settings
- Install Redis if needed:
  - **Windows**: Use WSL or Docker
  - **Mac**: `brew install redis && brew services start redis`
  - **Linux**: `sudo apt-get install redis-server && sudo systemctl start redis`

### Authentication fails
- Token may be expired (30 min default)
- Re-login to get a new token
- Check token format: `Bearer <token>`

## 💡 Tips

- **Auto-documentation**: Visit `/docs` for interactive API docs
- **Caching**: Preferences are cached for 1 hour in Redis
- **Token Management**: Use Postman environment variables for tokens
- **Testing**: Use `/internal/*` endpoints for service-to-service calls (no auth)

## 🎉 You're Ready!

Start testing the endpoints. For detailed examples, see `POSTMAN_TESTING.md`.

