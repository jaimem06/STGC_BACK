#!/usr/bin/env bash
# exit on error
set -o errexit

echo "Installing dependencies..."
NODE_ENV=development npm install

echo "Generating Prisma Client..."
npx prisma generate

echo "Building the TypeScript code..."
npm run build

echo "Build completed successfully!"