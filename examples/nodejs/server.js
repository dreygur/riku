/**
 * Riku Node.js Example Application
 * 
 * A simple Express.js application demonstrating Riku deployment.
 * 
 * Features:
 * - Health check endpoint
 * - Environment variable display
 * - Request logging
 * - Graceful shutdown
 */

const express = require('express');

const app = express();
const PORT = process.env.PORT || 3000;

// Middleware
app.use(express.json());
app.use(express.urlencoded({ extended: true }));

// Request logging middleware
app.use((req, res, next) => {
  const timestamp = new Date().toISOString();
  console.log(`[${timestamp}] ${req.method} ${req.path}`);
  next();
});

// Health check endpoint (used by Riku supervisor)
app.get('/health', (req, res) => {
  res.json({
    status: 'healthy',
    uptime: process.uptime(),
    timestamp: new Date().toISOString()
  });
});

// Root endpoint
app.get('/', (req, res) => {
  res.json({
    message: 'Welcome to Riku Node.js Example!',
    version: '1.0.0',
    documentation: 'https://dreygur.github.io/riku/'
  });
});

// Environment info endpoint
app.get('/env', (req, res) => {
  // Only expose safe environment variables
  const safeEnv = {
    NODE_ENV: process.env.NODE_ENV || 'development',
    PORT: process.env.PORT || '3000',
    HOSTNAME: process.env.HOSTNAME || 'unknown'
  };
  
  res.json({
    environment: safeEnv,
    nodeVersion: process.version,
    platform: process.platform
  });
});

// Echo endpoint (useful for testing)
app.post('/echo', (req, res) => {
  res.json({
    received: req.body,
    method: 'POST',
    path: '/echo'
  });
});

// 404 handler
app.use((req, res) => {
  res.status(404).json({
    error: 'Not Found',
    message: `Cannot ${req.method} ${req.path}`
  });
});

// Error handler
app.use((err, req, res, next) => {
  console.error(`[ERROR] ${err.message}`);
  res.status(500).json({
    error: 'Internal Server Error',
    message: process.env.NODE_ENV === 'production' 
      ? 'Something went wrong' 
      : err.message
  });
});

// Start server
const server = app.listen(PORT, '0.0.0.0', () => {
  console.log(`🚀 Server running on port ${PORT}`);
  console.log(`📝 Health check: http://localhost:${PORT}/health`);
});

// Graceful shutdown
process.on('SIGTERM', () => {
  console.log('📴 SIGTERM received, shutting down gracefully...');
  server.close(() => {
    console.log('✅ Server closed');
    process.exit(0);
  });
  
  // Force close after 10 seconds
  setTimeout(() => {
    console.error('⚠️  Forced shutdown');
    process.exit(1);
  }, 10000);
});

process.on('SIGINT', () => {
  console.log('📴 SIGINT received, shutting down gracefully...');
  server.close(() => {
    console.log('✅ Server closed');
    process.exit(0);
  });
});

module.exports = app;
// Updated via git push
