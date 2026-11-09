# Etapa: instalar dependencias y compilar el shared package
FROM node:24-alpine AS deps
WORKDIR /app
COPY package.json package-lock.json* ./
COPY apps/api/package.json apps/api/package.json
COPY apps/web/package.json apps/web/package.json
COPY packages/shared/package.json packages/shared/package.json
RUN npm ci --include=dev
COPY . .
RUN npm run build -w @parte/shared

# Etapa runtime: ejecutar la API con tsx (código TypeScript)
FROM node:24-alpine AS runtime
WORKDIR /app
ENV NODE_ENV=production

COPY --from=deps /app/package.json /app/package-lock.json* ./
COPY --from=deps /app/apps/api/package.json apps/api/package.json
COPY --from=deps /app/packages/shared/package.json packages/shared/package.json

RUN npm ci --omit=dev --workspace @parte/api --workspace @parte/shared

COPY --from=deps /app/apps/api apps/api
COPY --from=deps /app/packages/shared packages/shared
COPY --from=deps /app/scripts scripts

WORKDIR /app/apps/api
EXPOSE 3001
CMD ["npx", "tsx", "src/index.ts"]