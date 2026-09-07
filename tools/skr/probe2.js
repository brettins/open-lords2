const fs=require("fs");
const b=fs.readFileSync("F:/games/Lords of the Realm II/USER.SKR");
const start=5420, data=b.subarray(start);
console.log("data len",data.length);
// byte histogram
const h=new Array(256).fill(0);
for(const v of data) h[v]++;
console.log("distinct bytes:", h.map((c,i)=>[i,c]).filter(x=>x[1]).map(x=>x[0]+":"+x[1]).join(" "));
// autocorrelation: match fraction at lag
function score(lag){let m=0,n=data.length-lag;for(let i=0;i<n;i++) if(data[i]===data[i+lag])m++;return m/n;}
let best=[];
for(let lag=1;lag<=12000;lag++) best.push([lag,score(lag)]);
best.sort((a,b)=>b[1]-a[1]);
console.log("top lags:", best.slice(0,25).map(x=>x[0]+"="+x[1].toFixed(4)).join(" "));
