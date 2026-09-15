// Isolated AV smoke client; pair with DEN_BIND=127.0.0.1:7014 and DEN_ORIGIN=http://localhost:5178.
import config from './vite.config.ts'
export default { ...config, server: { ...config.server, port: 5178,
  proxy: Object.fromEntries(Object.entries(config.server!.proxy!).map(([key, value]) => [key, typeof value === 'object' ? { ...value, target: String(value.target).replace(':7000', ':7014') } : value]))
} }
