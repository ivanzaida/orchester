# The web front of Orchester Cloud (docs/architecture/server.md, "The cloud's web front"). Build from web/cloud:
#   docker build -t orchester-cloud-web web/cloud
# Configuration comes only from the environment at run time: ORCHESTER_CLOUD_PUBLIC_URL, ORCHESTER_CLOUD_API_URL
# and ORCHESTER_CLOUD_WEB_SECRET (32 random bytes in base64).
FROM node:24-alpine AS build
WORKDIR /app
ENV NEXT_TELEMETRY_DISABLED=1
COPY package.json package-lock.json ./
RUN npm ci --ignore-scripts --no-audit --no-fund
COPY . .
RUN npx next build

FROM node:24-alpine
WORKDIR /app
ENV NODE_ENV=production NEXT_TELEMETRY_DISABLED=1 HOSTNAME=0.0.0.0 PORT=3000
COPY --from=build /app/.next/standalone ./
COPY --from=build /app/.next/static ./.next/static
COPY --from=build /app/public ./public
USER node
EXPOSE 3000
CMD ["node", "server.js"]
