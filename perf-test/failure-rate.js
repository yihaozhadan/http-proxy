import http from 'k6/http';
import { check } from 'k6';
import { Counter } from 'k6/metrics';

// Custom metrics for status codes
const successCount = new Counter('http_200_responses');
const errorCount = new Counter('http_500_responses');

export const options = {
    scenarios: {
        constant_iterations: {
            executor: 'shared-iterations',
            vus: 10,
            iterations: 1000,
            maxDuration: '2m'
        }
    }
};

export default function() {
    const payload = JSON.stringify({
        event: 'test_webhook',
        timestamp: Date.now()
    });

    const params = {
        headers: {
            'Content-Type': 'application/json'
        }
    };

    const response = http.post('http://localhost:3000/proxy', payload, params);

    check(response, {
        'status is 200 or 500': (r) => [200, 500].includes(r.status)
    });

    // Count status codes
    if (response.status === 200) {
        successCount.add(1);
    } else if (response.status === 500) {
        errorCount.add(1);
    }
}