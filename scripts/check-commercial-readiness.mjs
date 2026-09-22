// Read-only preflight. Never prints environment values, tokens, or response bodies.
import { readFileSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
const root = fileURLToPath(new URL('../', import.meta.url));
let failures = 0;
function check(ok, label) { console.log(`${ok ? 'OK' : 'MISSING'} ${label}`); if (!ok) failures++; }
const read = path => readFileSync(resolve(root, path), 'utf8');
const poly = 'FFCCA38841ADB694B6F380647E15F17C446A4D1656FED51A1E2041D064C94CC8';
const apache = 'CFC7749B96F63BD31C3C42B5C471BF756814053E847C10F3EB003417BC523D30';
for (const [path, hash] of [['LICENSE',poly],['packages/api-client/LICENSE',apache],['packages/contracts/LICENSE',apache]]) {
  check(existsSync(resolve(root,path)) && createHash('sha256').update(readFileSync(resolve(root,path))).digest('hex').toUpperCase()===hash, `unmodified standard license: ${path}`);
}
check(/license\s*=\s*"PolyForm-Noncommercial-1.0.0"/.test(read('Cargo.toml')), 'Rust workspace license');
for (const [path, license] of [['package.json','PolyForm-Noncommercial-1.0.0'],['apps/desktop/package.json','PolyForm-Noncommercial-1.0.0'],['packages/ui-tokens/package.json','PolyForm-Noncommercial-1.0.0'],['packages/api-client/package.json','Apache-2.0'],['packages/contracts/package.json','Apache-2.0']]) {
  check(JSON.parse(read(path)).license===license, `package license: ${path}`);
}
if (process.argv.includes('--deployment')) {
  const names=['ARO_STRIPE_SECRET_KEY','ARO_STRIPE_WEBHOOK_SECRET','ARO_STRIPE_PRICE_CLOUD','ARO_STRIPE_PRICE_BUSINESS','ARO_BILLING_RETURN_URL','ARO_SALES_EMAIL'];
  for(const name of names) check(Boolean(process.env[name]?.trim()),name);
  for(const name of ['ARO_COMMERCIAL_TERMS_APPROVED','ARO_COMMERCIAL_ENFORCEMENT']) check(process.env[name]==='true',name);
  const operatorPath=resolve(root,'legal/operator.json');
  let operator={};
  if(existsSync(operatorPath)) {try {operator=JSON.parse(readFileSync(operatorPath,'utf8'));} catch {check(false,'valid legal/operator.json');}}
  for(const field of ['legalName','businessAddress','country','contactEmail','termsUrl','privacyUrl']) check(typeof operator[field]==='string' && operator[field].trim().length>0,`operator identity: ${field}`);
  for(const [label,value] of [['return URL',process.env.ARO_BILLING_RETURN_URL],['terms URL',operator.termsUrl],['privacy URL',operator.privacyUrl]]) {
    let valid=false;try {const u=new URL(value);valid=u.protocol==='https:' && !u.username && !u.password && !u.hostname.endsWith('.example') && !u.hostname.endsWith('.test');}catch {}
    check(valid,`production HTTPS ${label}`);
  }
  check(operator.contractsFinalized===true,'operator attestation: contracts finalized');
  check(operator.modelRightsVerified===true,'operator attestation: hosted model rights verified');
  check(process.env.ARO_COMPUTE_ENABLED==='true','ARO_COMPUTE_ENABLED');
  let models=[];try{models=JSON.parse(process.env.ARO_COMPUTE_MODELS_JSON??'[]');}catch{}
  check(Array.isArray(models)&&models.length>0,'managed model catalog');
  check(Boolean(process.env.ARO_COMPUTE_BASE_URL),'ARO_COMPUTE_BASE_URL');
  console.log('Operator attestations are declarations, not independent legal or operational verification.');
}
if(process.argv.includes('--stripe-test')) {
  const secret=process.env.ARO_STRIPE_SECRET_KEY??'';
  if(!/^(sk|rk)_test_/.test(secret)) check(false,'Stripe TEST key required; live keys are refused by this check');
  else {
    for(const [plan,amount,tax] of [['CLOUD',1200,'inclusive'],['BUSINESS',2900,'exclusive']]) {
      const id=process.env[`ARO_STRIPE_PRICE_${plan}`]??'';
      if(!/^price_[A-Za-z0-9]+$/.test(id)) {check(false,`Stripe ${plan} price ID`);continue;}
      try {
        const response=await fetch(`https://api.stripe.com/v1/prices/${id}`,{headers:{Authorization:`Bearer ${secret}`,'Stripe-Version':'2025-02-24.acacia'},redirect:'error',signal:AbortSignal.timeout(15000)});
        const value=await response.json();
        check(response.ok && value.livemode===false && value.active===true && value.currency==='eur' && value.unit_amount===amount && value.tax_behavior===tax && value.recurring?.interval==='month' && value.recurring?.interval_count===1,`Stripe TEST ${plan}: catalog amount, tax and interval`);
      } catch {check(false,`Stripe TEST ${plan}: provider reachable`);}
    }
  }
  console.log('This read-only check does not test a payment, refund, or webhook delivery.');
}
console.log(`${failures} missing or invalid check(s).`);
process.exitCode=failures?1:0;
