#!/usr/bin/env bash
# exit on error
set -o errexit

echo "Installing dependencies..."
npm ci

echo "Generating Prisma Client..."
npx prisma generate

echo "Building the TypeScript code..."
npm run build

echo "Build completed successfully!"