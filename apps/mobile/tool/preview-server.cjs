// Local development only: serves the Flutter build and proxies ARO without CORS.
// This server is never bundled in the mobile application.
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '../build/web');
const target = new URL(process.env.ARO_PREVIEW_API || 'http://127.0.0.1:8710');
if (!['127.0.0.1', 'localhost', '[::1]'].includes(target.hostname)) throw new Error('Preview API must be local.');
const mime = {'.html':'text/html','.js':'text/javascript','.json':'application/json','.wasm':'application/wasm','.png':'image/png','.ttf':'font/ttf','.woff2':'font/woff2','.css':'text/css'};
http.createServer((req, res) => {
  if (req.url.startsWith('/v1/') || req.url === '/health') {
    const proxy = http.request({hostname:target.hostname,port:target.port || 80,path:req.url,method:req.method,headers:{...req.headers,host:target.host}}, incoming => {
      res.writeHead(incoming.statusCode,incoming.headers);incoming.pipe(res);
    });
    proxy.on('error',()=>{res.writeHead(502,{'Content-Type':'application/json'});res.end(JSON.stringify({error:'API locale indisponible'}));});
    req.pipe(proxy);res.on('close',()=>proxy.destroy());return;
  }
  let pathname;
  try { pathname = decodeURIComponent(new URL(req.url,'http://localhost').pathname); } catch {res.writeHead(400);res.end();return;}
  let file = path.resolve(root, '.' + pathname);
  if (file !== root && !file.startsWith(root + path.sep)) {res.writeHead(403);res.end();return;}
  if (!fs.existsSync(file) || !fs.statSync(file).isFile()) file = path.join(root,'index.html');
  if (!fs.existsSync(file)) {res.writeHead(503);res.end('Build Flutter requis.');return;}
  res.writeHead(200,{'Content-Type':mime[path.extname(file)] || 'application/octet-stream','Cache-Control':'no-store'});fs.createReadStream(file).pipe(res);
}).listen(1440,'127.0.0.1',()=>console.log('ARO mobile preview: http://127.0.0.1:1440'));
