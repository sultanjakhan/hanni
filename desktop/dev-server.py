import http.server, functools, os
os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'src'))
http.server.HTTPServer(('127.0.0.1', 3000),
    functools.partial(http.server.SimpleHTTPRequestHandler, directory='.')).serve_forever()
