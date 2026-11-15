# Web landing build for Coolify (copyspeak.ilyaizen.link).
# COPYSPEAK_WEB=1 sets VITE_IS_WEB=true (vite.config.js define), which selects the landing page.
# Without it, the image ships the Play UI.
FROM oven/bun:1.4.4 AS build
WORKDIR /app
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile
COPY . .
ENV COPYSPEAK_WEB=1
RUN bun run build

FROM nginx:1.29-alpine
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/dist /usr/share/nginx/html
# Fail the image build on a bad nginx config, not at the first request.
RUN nginx -t
EXPOSE 80
