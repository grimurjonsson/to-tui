"""Check overlapping browser login attempts without signing into Google."""

import argparse
import http.cookiejar
import urllib.error
import urllib.parse
import urllib.request


class StopRedirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', default='https://totui.gimmi.is/')
    parser.add_argument('--expected-return', default='https://totui.gimmi.is/')
    args = parser.parse_args()
    jar = http.cookiejar.CookieJar()
    client = urllib.request.build_opener(StopRedirects(), urllib.request.HTTPCookieProcessor(jar))
    first = None
    for attempt in range(3):
        try:
            response = client.open(args.base, timeout=15)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            assert response.status == 302, 'Expected a login redirect'
            query = urllib.parse.parse_qs(urllib.parse.urlsplit(response.headers['Location']).query)
            assert query.get('state'), 'Login redirect has no state'
            destination = query['state'][0].partition(':')[2]
            assert destination == args.expected_return, 'Login would return to the wrong application'
            callback = query['redirect_uri'][0]
        cookies = {cookie.name: cookie.value for cookie in jar if '_csrf' in cookie.name}
        assert cookies, 'Login did not set a CSRF cookie'
        if first is None:
            first = cookies.copy()
        else:
            assert all(cookies.get(name) == value for name, value in first.items()), (
                'A later login overwrote the first attempt’s CSRF cookie'
            )
            assert len(cookies) == attempt + 1, 'Login attempts must have separate CSRF cookies'
        request = urllib.request.Request(callback)
        jar.add_cookie_header(request)
        sent = request.get_header('Cookie', '')
        assert all(f'{name}={value}' in sent for name, value in first.items()), (
            'The first login cookie would not reach the callback domain'
        )
    print('PASS: Three overlapping login attempts retain separate CSRF cookies, all sent to the callback domain, with the correct application return URL.')


if __name__ == '__main__':
    main()
