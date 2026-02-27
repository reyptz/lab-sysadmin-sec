# DevStack SRE Monitoring & Infrastructure

## Overview

This is a comprehensive SRE (Site Reliability Engineering) monitoring and infrastructure stack designed for development environments. It includes observability, security, CI/CD, and big data components with production-ready configurations.

## Architecture

### Core Services
- **Zookeeper** - Distributed coordination service
- **Kafka** - Distributed streaming platform
- **RabbitMQ** - Message broker
- **Redis** - In-memory data store
- **PostgreSQL** - Primary database
- **MongoDB** - NoSQL database

### Observability Stack
- **Prometheus** - Metrics collection and alerting
- **Grafana** - Visualization and dashboards
- **Loki** - Log aggregation
- **Elasticsearch** - Search and analytics
- **Kibana** - Elasticsearch visualization
- **Jaeger** - Distributed tracing
- **Tempo** - Grafana tracing backend
- **Alertmanager** - Alert routing and management
- **OpenTelemetry Collector** - Telemetry data collection

### Security & Service Discovery
- **Keycloak** - Identity and access management
- **Vault** - Secrets management
- **Consul** - Service discovery and configuration

### CI/CD & Quality
- **Jenkins** - Continuous integration
- **SonarQube** - Code quality analysis

### API Gateway & Reverse Proxy
- **Traefik** - Modern reverse proxy and load balancer
- **Nginx** - High-performance web server
- **Envoy** - Service mesh proxy

### Big Data & Workflow
- **Airflow** - Workflow orchestration
- **Spark** - Big data processing
- **Logstash** - Log processing
- **Splunk** - Security information and event management

### Automation
- **Ansible** - Configuration management

## Quick Start

### Prerequisites
- Docker Desktop
- Docker Compose
- At least 8GB RAM
- 20GB free disk space

### Starting Services

#### Core Services
```bash
docker compose --profile core up -d
```

#### Messaging
```bash
docker compose --profile messaging up -d
```

#### Observability
```bash
docker compose --profile observability up -d
```

#### Security
```bash
docker compose --profile security up -d
```

#### CI/CD
```bash
docker compose --profile ci up -d
```

#### Frontend/Proxy
```bash
docker compose --profile frontend up -d
```

#### Big Data
```bash
docker compose --profile bigdata up -d
```

#### All Services
```bash
docker compose --profile core --profile messaging --profile observability --profile security --profile ci --profile frontend --profile bigdata up -d
```

### Monitoring Extensions

For additional monitoring capabilities:

```bash
docker compose -f docker-compose.monitoring.yml --profile monitoring up -d
```

## Access Points

### Web Interfaces

| Service             |           URL          |
|---------------------|------------------------|
| Grafana             | http://localhost:3000  |
| Prometheus          | http://localhost:9090  |
| Kibana              | http://localhost:5601  |
| Traefik Dashboard   | http://localhost:8080  |
| Jenkins             | http://localhost:8082  |
| SonarQube           | http://localhost:9000  |
| Keycloak            | http://localhost:8081  |
| Vault               | http://localhost:8200  |
| Consul              | http://localhost:8500  |
| RabbitMQ Management | http://localhost:15672 |
| Airflow             | http://localhost:8083  |
| Spark Master        | http://localhost:8084  |
| Splunk              | http://localhost:8000  |
| Jaeger              | http://localhost:16686 |

### API Endpoints

| Service       | Port      | Protocol  |
|---------------|-----------|-----------|
| Kafka         | 19092     | TCP       |
| Redis         | 6379      | TCP       |
| PostgreSQL    | 5433      | TCP       |
| MongoDB       | 27017     | TCP       |
| Elasticsearch | 9200      | HTTP      |
| Loki          | 3100      | HTTP      |
| OpenTelemetry | 4317/4318 | gRPC/HTTP |

## Configuration Files

### Monitoring Configuration
- `prometheus.yml` - Prometheus scraping configuration
- `prometheus-rules.yml` - Alerting rules
- `alertmanager.yml` - Alert routing
- `loki-config.yml` - Log aggregation
- `grafana-datasources.yml` - Grafana datasources

### Service Configuration
- `envoy.yaml` - Envoy proxy routing
- `nginx.conf` - Nginx reverse proxy
- `logstash.conf` - Log processing
- `otel-collector-config.yaml` - OpenTelemetry collector

### Monitoring Extensions
- `docker-compose.monitoring.yml` - Additional monitoring services
- `jmx-exporter-config.yml` - JMX metrics
- `blackbox-config.yml` - Blackbox probing
- `promtail-config.yml` - Log collection
- `tempo-config.yml` - Distributed tracing

## SRE Features

### Health Checks
All services include comprehensive health checks with appropriate intervals and timeouts.

### Resource Limits
Memory and CPU limits are configured for all services to prevent resource exhaustion.

### Security
- `no-new-privileges:true` for all containers
- Non-default passwords
- Network segmentation
- Security headers in Nginx

### Observability
- Comprehensive metrics collection
- Structured logging
- Distributed tracing
- Alerting rules for all critical components

### High Availability
- Service dependencies with health checks
- Graceful shutdown handling
- Restart policies

## Monitoring Dashboards

### Pre-configured Alerts
- High CPU/Memory usage
- Service availability
- Disk space low
- Database connection issues
- Queue size warnings
- SSL certificate expiry

### Key Metrics
- System resources (CPU, Memory, Disk)
- Container health and performance
- Application response times
- Error rates and availability
- Database performance
- Message queue metrics

## Troubleshooting

### Common Issues

#### Services Not Starting
```bash
# Check logs
docker compose logs <service-name>

# Check resource usage
docker stats

# Check network connectivity
docker compose exec <service-name> ping <other-service>
```

#### Health Check Failures
```bash
# Check health status
docker compose ps

# Manual health check
docker compose exec <service-name> curl http://localhost:<port>/health
```

#### Resource Issues
```bash
# Check memory usage
docker stats --no-stream

# Clean up unused resources
docker system prune -f
```

### Logs

#### Application Logs
```bash
# View service logs
docker compose logs -f <service-name>

# View all logs
docker compose logs -f
```

#### System Logs
```bash
# Nginx access logs
docker compose exec nginx tail -f /var/log/nginx/access.log

# Prometheus logs
docker compose exec prometheus tail -f /var/log/prometheus.log
```

## Development

### Adding New Services

1. Add service to `docker-compose.yml`
2. Configure health checks
3. Add monitoring rules to `prometheus-rules.yml`
4. Update Prometheus scraping config
5. Add Grafana dashboard

### Custom Metrics

1. Configure OpenTelemetry collector
2. Add Prometheus exporter
3. Create alerting rules
4. Build Grafana dashboards

### Security Hardening

1. Review container security options
2. Update network policies
3. Rotate secrets regularly
4. Enable audit logging

## Production Considerations

### Scaling
- Use Docker Swarm or Kubernetes
- Configure external storage
- Implement proper backup strategies
- Set up external monitoring

### Security
- Enable TLS everywhere
- Use external secret management
- Implement network policies
- Regular security updates

### Performance
- Tune resource limits
- Optimize database configurations
- Implement caching strategies
- Monitor and optimize queries

## Support

### Documentation
- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Loki Documentation](https://grafana.com/docs/loki/latest/)
- [Docker Compose Documentation](https://docs.docker.com/compose/)

### Community
- [SRE Community](https://www.usenix.org/conferences/srecon)
- [Observability Community](https://grafana.com/blog/)
- [Docker Community](https://www.docker.com/community)

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## Changelog

### v1.0.0
- Initial SRE stack implementation
- Comprehensive monitoring setup
- Security hardening
- Documentation and guides
