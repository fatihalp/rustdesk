function getCsrfToken() {
  const meta = document.querySelector('meta[name="csrf-token"]');
  return meta ? meta.getAttribute('content') : window.__CSRF_TOKEN__ || '';
}

export async function request(endpoint, options = {}) {
  const url = endpoint;
  const headers = {
    'Accept': 'application/json',
    'X-CSRF-TOKEN': getCsrfToken(),
    'X-Requested-With': 'XMLHttpRequest',
    ...options.headers,
  };

  if (!(options.body instanceof FormData) && options.body && typeof options.body === 'object') {
    headers['Content-Type'] = 'application/json';
    options.body = JSON.stringify(options.body);
  }

  const response = await fetch(url, {
    credentials: 'same-origin',
    ...options,
    headers,
  });

  const contentType = response.headers.get('content-type') || '';
  let data = null;
  if (contentType.includes('application/json')) {
    data = await response.json();
  } else {
    data = await response.text();
  }

  if (!response.ok) {
    const error = new Error(data?.message || data?.error || 'An error occurred.');
    error.status = response.status;
    error.data = data;
    throw error;
  }

  return data;
}

export const api = {
  get: (endpoint) => request(endpoint, { method: 'GET' }),
  post: (endpoint) => request(endpoint, { method: 'POST' }),
  postJson: (endpoint, body) => request(endpoint, { method: 'POST', body }),
  put: (endpoint, body) => request(endpoint, { method: 'PUT', body }),
  delete: (endpoint) => request(endpoint, { method: 'DELETE' }),
  postForm: (endpoint, formData) => request(endpoint, { method: 'POST', body: formData }),
};
// Preserve api.post(endpoint, body) signature used across components
api.post = (endpoint, body) => request(endpoint, { method: 'POST', body });
