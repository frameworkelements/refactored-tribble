# ---- Build stage ---------------------------------------------------------
FROM node:22-alpine AS builder

WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm ci || npm install
COPY . .
RUN npm run build

# ---- Runtime stage (nginx) ----------------------------------------------
FROM nginx:1.27-alpine AS runtime

# Replace default config with our SPA + /api proxy configuration.
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html

# nginx:alpine ships a non-root capable setup. Run worker processes as the
# unprivileged "nginx" user and listen on an unprivileged port (8080).
RUN touch /var/run/nginx.pid \
    && chown -R nginx:nginx /var/run/nginx.pid /usr/share/nginx/html /var/cache/nginx

USER nginx
EXPOSE 8080

HEALTHCHECK --interval=15s --timeout=3s --retries=5 \
    CMD wget -qO- http://localhost:8080/healthz || exit 1

CMD ["nginx", "-g", "daemon off;"]
