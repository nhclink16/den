// Isolated sounds smoke client; pair with DEN_BIND=127.0.0.1:7018 and DEN_ORIGIN=http://localhost:5182.
import config from './vite.config.ts'
export default { ...config, server: { ...config.server, port: 5182,
  proxy: Object.fromEntries(Object.entries(config.server!.proxy!).map(([key, value]) => [key, typeof value === 'object' ? { ...value, target: String(value.target).replace(':7000', ':7018') } : value]))
} }
