const __vite__fileDeps=["assets/index-ZuaO7Tjw.js","assets/index-DzA96B0X.js","assets/index-CZgkhW69.css"],__vite__mapDeps=i=>i.map(i=>__vite__fileDeps[i]);
import{_ as e}from"./index-DzA96B0X.js";const t=Symbol(),s=Object.getPrototypeOf,o=new WeakMap,n=e=>(e=>e&&(o.has(e)?o.get(e):s(e)===Object.prototype||s(e)===Array.prototype))(e)&&e[t]||null,a=(e,t=!0)=>{o.set(e,t)};var r={BASE_URL:"/",MODE:"production",DEV:!1,PROD:!0,SSR:!1};const i=e=>"object"==typeof e&&null!==e,l=new WeakMap,c=new WeakSet,[d]=((e=Object.is,t=((e,t)=>new Proxy(e,t)),s=(e=>i(e)&&!c.has(e)&&(Array.isArray(e)||!(Symbol.iterator in e))&&!(e instanceof WeakMap)&&!(e instanceof WeakSet)&&!(e instanceof Error)&&!(e instanceof Number)&&!(e instanceof Date)&&!(e instanceof String)&&!(e instanceof RegExp)&&!(e instanceof ArrayBuffer)),o=(e=>{switch(e.status){case"fulfilled":return e.value;case"rejected":throw e.reason;default:throw e}}),d=new WeakMap,p=((e,t,s=o)=>{const n=d.get(e);if((null==n?void 0:n[0])===t)return n[1];const r=Array.isArray(e)?[]:Object.create(Object.getPrototypeOf(e));return a(r,!0),d.set(e,[t,r]),Reflect.ownKeys(e).forEach((t=>{if(Object.getOwnPropertyDescriptor(r,t))return;const o=Reflect.get(e,t),n={value:o,enumerable:!0,configurable:!0};if(c.has(o))a(o,!1);else if(o instanceof Promise)delete n.value,n.get=()=>s(o);else if(l.has(o)){const[e,t]=l.get(o);n.value=p(e,t(),s)}Object.defineProperty(r,t,n)})),Object.preventExtensions(r)}),u=new WeakMap,g=[1,1],m=(o=>{if(!i(o))throw new Error("object required");const a=u.get(o);if(a)return a;let d=g[0];const h=new Set,f=(e,t=++g[0])=>{d!==t&&(d=t,h.forEach((s=>s(e,t))))};let b=g[1];const y=e=>(t,s)=>{const o=[...t];o[1]=[e,...o[1]],f(o,s)},v=new Map,w=e=>{var t;const s=v.get(e);s&&(v.delete(e),null==(t=s[1])||t.call(s))},I=Array.isArray(o)?[]:Object.create(Object.getPrototypeOf(o)),O=t(I,{deleteProperty(e,t){const s=Reflect.get(e,t);w(t);const o=Reflect.deleteProperty(e,t);return o&&f(["delete",[t],s]),o},set(t,o,a,d){const p=Reflect.has(t,o),g=Reflect.get(t,o,d);if(p&&(e(g,a)||u.has(a)&&e(g,u.get(a))))return!0;w(o),i(a)&&(a=n(a)||a);let b=a;if(a instanceof Promise)a.then((e=>{a.status="fulfilled",a.value=e,f(["resolve",[o],e])})).catch((e=>{a.status="rejected",a.reason=e,f(["reject",[o],e])}));else{!l.has(a)&&s(a)&&(b=m(a));const e=!c.has(b)&&l.get(b);e&&((e,t)=>{if("production"!==(r?"production":void 0)&&v.has(e))throw new Error("prop listener already exists");if(h.size){const s=t[3](y(e));v.set(e,[t,s])}else v.set(e,[t])})(o,e)}return Reflect.set(t,o,b,d),f(["set",[o],a,g]),!0}});u.set(o,O);const C=[I,(e=++g[1])=>(b===e||h.size||(b=e,v.forEach((([t])=>{const s=t[1](e);s>d&&(d=s)}))),d),p,e=>{h.add(e),1===h.size&&v.forEach((([e,t],s)=>{if("production"!==(r?"production":void 0)&&t)throw new Error("remove already exists");const o=e[3](y(s));v.set(s,[e,o])}));return()=>{h.delete(e),0===h.size&&v.forEach((([e,t],s)=>{t&&(t(),v.set(s,[e]))}))}}];return l.set(O,C),Reflect.ownKeys(o).forEach((e=>{const t=Object.getOwnPropertyDescriptor(o,e);"value"in t&&(O[e]=o[e],delete t.value,delete t.writable),Object.defineProperty(I,e,t)})),O}))=>[m,l,c,e,t,s,o,d,p,u,g])();function p(e={}){return d(e)}function u(e,t,s){const o=l.get(e);let n;"production"===(r?"production":void 0)||o||console.warn("Please use proxy object");const a=[],i=o[3];let c=!1;const d=i((e=>{a.push(e),n||(n=Promise.resolve().then((()=>{n=void 0,c&&t(a.splice(0))})))}));return c=!0,()=>{c=!1,d()}}const g=p({history:["ConnectWallet"],view:"ConnectWallet",data:void 0}),m={state:g,subscribe:e=>u(g,(()=>e(g))),push(e,t){e!==g.view&&(g.view=e,t&&(g.data=t),g.history.push(e))},reset(e){g.view=e,g.history=[e]},replace(e){g.history.length>1&&(g.history[g.history.length-1]=e,g.view=e)},goBack(){if(g.history.length>1){g.history.pop();const[e]=g.history.slice(-1);g.view=e}},setData(e){g.data=e}},h={WALLETCONNECT_DEEPLINK_CHOICE:"WALLETCONNECT_DEEPLINK_CHOICE",WCM_VERSION:"WCM_VERSION",RECOMMENDED_WALLET_AMOUNT:9,isMobile:()=>typeof window<"u"&&Boolean(window.matchMedia("(pointer:coarse)").matches||/Android|webOS|iPhone|iPad|iPod|BlackBerry|Opera Mini/u.test(navigator.userAgent)),isAndroid:()=>h.isMobile()&&navigator.userAgent.toLowerCase().includes("android"),isIos(){const e=navigator.userAgent.toLowerCase();return h.isMobile()&&(e.includes("iphone")||e.includes("ipad"))},isHttpUrl:e=>e.startsWith("http://")||e.startsWith("https://"),isArray:e=>Array.isArray(e)&&e.length>0,formatNativeUrl(e,t,s){if(h.isHttpUrl(e))return this.formatUniversalUrl(e,t,s);let o=e;o.includes("://")||(o=e.replaceAll("/","").replaceAll(":",""),o=`${o}://`),o.endsWith("/")||(o=`${o}/`),this.setWalletConnectDeepLink(o,s);return`${o}wc?uri=${encodeURIComponent(t)}`},formatUniversalUrl(e,t,s){if(!h.isHttpUrl(e))return this.formatNativeUrl(e,t,s);let o=e;o.endsWith("/")||(o=`${o}/`),this.setWalletConnectDeepLink(o,s);return`${o}wc?uri=${encodeURIComponent(t)}`},wait:async e=>new Promise((t=>{setTimeout(t,e)})),openHref(e,t){window.open(e,t,"noreferrer noopener")},setWalletConnectDeepLink(e,t){try{localStorage.setItem(h.WALLETCONNECT_DEEPLINK_CHOICE,JSON.stringify({href:e,name:t}))}catch{console.info("Unable to set WalletConnect deep link")}},setWalletConnectAndroidDeepLink(e){try{const[t]=e.split("?");localStorage.setItem(h.WALLETCONNECT_DEEPLINK_CHOICE,JSON.stringify({href:t,name:"Android"}))}catch{console.info("Unable to set WalletConnect android deep link")}},removeWalletConnectDeepLink(){try{localStorage.removeItem(h.WALLETCONNECT_DEEPLINK_CHOICE)}catch{console.info("Unable to remove WalletConnect deep link")}},setModalVersionInStorage(){try{typeof localStorage<"u"&&localStorage.setItem(h.WCM_VERSION,"2.6.2")}catch{console.info("Unable to set Web3Modal version in storage")}},getWalletRouterData(){var e;const t=null==(e=m.state.data)?void 0:e.Wallet;if(!t)throw new Error('Missing "Wallet" view data');return t}},f=p({enabled:typeof location<"u"&&(location.hostname.includes("localhost")||location.protocol.includes("https")),userSessionId:"",events:[],connectedWalletId:void 0}),b={state:f,subscribe:e=>u(f.events,(()=>e(function(e,t){const s=l.get(e);"production"===(r?"production":void 0)||s||console.warn("Please use proxy object");const[o,n,a]=s;return a(o,n(),t)}(f.events[f.events.length-1])))),initialize(){f.enabled&&typeof(null==crypto?void 0:crypto.randomUUID)<"u"&&(f.userSessionId=crypto.randomUUID())},setConnectedWalletId(e){f.connectedWalletId=e},click(e){if(f.enabled){const t={type:"CLICK",name:e.name,userSessionId:f.userSessionId,timestamp:Date.now(),data:e};f.events.push(t)}},track(e){if(f.enabled){const t={type:"TRACK",name:e.name,userSessionId:f.userSessionId,timestamp:Date.now(),data:e};f.events.push(t)}},view(e){if(f.enabled){const t={type:"VIEW",name:e.name,userSessionId:f.userSessionId,timestamp:Date.now(),data:e};f.events.push(t)}}},y=p({chains:void 0,walletConnectUri:void 0,isAuth:!1,isCustomDesktop:!1,isCustomMobile:!1,isDataLoaded:!1,isUiLoaded:!1}),v={state:y,subscribe:e=>u(y,(()=>e(y))),setChains(e){y.chains=e},setWalletConnectUri(e){y.walletConnectUri=e},setIsCustomDesktop(e){y.isCustomDesktop=e},setIsCustomMobile(e){y.isCustomMobile=e},setIsDataLoaded(e){y.isDataLoaded=e},setIsUiLoaded(e){y.isUiLoaded=e},setIsAuth(e){y.isAuth=e}},w=p({projectId:"",mobileWallets:void 0,desktopWallets:void 0,walletImages:void 0,chains:void 0,enableAuthMode:!1,enableExplorer:!0,explorerExcludedWalletIds:void 0,explorerRecommendedWalletIds:void 0,termsOfServiceUrl:void 0,privacyPolicyUrl:void 0}),I={state:w,subscribe:e=>u(w,(()=>e(w))),setConfig(e){var t,s;b.initialize(),v.setChains(e.chains),v.setIsAuth(Boolean(e.enableAuthMode)),v.setIsCustomMobile(Boolean(null==(t=e.mobileWallets)?void 0:t.length)),v.setIsCustomDesktop(Boolean(null==(s=e.desktopWallets)?void 0:s.length)),h.setModalVersionInStorage(),Object.assign(w,e)}};var O=Object.defineProperty,C=Object.getOwnPropertySymbols,E=Object.prototype.hasOwnProperty,W=Object.prototype.propertyIsEnumerable,L=(e,t,s)=>t in e?O(e,t,{enumerable:!0,configurable:!0,writable:!0,value:s}):e[t]=s;const j="https://explorer-api.walletconnect.com",A="wcm",M="js-2.6.2";async function U(e,t){const s=((e,t)=>{for(var s in t||(t={}))E.call(t,s)&&L(e,s,t[s]);if(C)for(var s of C(t))W.call(t,s)&&L(e,s,t[s]);return e})({sdkType:A,sdkVersion:M},t),o=new URL(e,j);return o.searchParams.append("projectId",I.state.projectId),Object.entries(s).forEach((([e,t])=>{t&&o.searchParams.append(e,String(t))})),(await fetch(o)).json()}const D={getDesktopListings:async e=>U("/w3m/v1/getDesktopListings",e),getMobileListings:async e=>U("/w3m/v1/getMobileListings",e),getInjectedListings:async e=>U("/w3m/v1/getInjectedListings",e),getAllListings:async e=>U("/w3m/v1/getAllListings",e),getWalletImageUrl:e=>`${j}/w3m/v1/getWalletImage/${e}?projectId=${I.state.projectId}&sdkType=${A}&sdkVersion=${M}`,getAssetImageUrl:e=>`${j}/w3m/v1/getAssetImage/${e}?projectId=${I.state.projectId}&sdkType=${A}&sdkVersion=${M}`};var k=Object.defineProperty,P=Object.getOwnPropertySymbols,S=Object.prototype.hasOwnProperty,_=Object.prototype.propertyIsEnumerable,N=(e,t,s)=>t in e?k(e,t,{enumerable:!0,configurable:!0,writable:!0,value:s}):e[t]=s;const x=h.isMobile(),R=p({wallets:{listings:[],total:0,page:1},search:{listings:[],total:0,page:1},recomendedWallets:[]}),T={state:R,async getRecomendedWallets(){const{explorerRecommendedWalletIds:e,explorerExcludedWalletIds:t}=I.state;if("NONE"===e||"ALL"===t&&!e)return R.recomendedWallets;if(h.isArray(e)){const t={recommendedIds:e.join(",")},{listings:s}=await D.getAllListings(t),o=Object.values(s);o.sort(((t,s)=>e.indexOf(t.id)-e.indexOf(s.id))),R.recomendedWallets=o}else{const{chains:e,isAuth:s}=v.state,o=null==e?void 0:e.join(","),n=h.isArray(t),a={page:1,sdks:s?"auth_v1":void 0,entries:h.RECOMMENDED_WALLET_AMOUNT,chains:o,version:2,excludedIds:n?t.join(","):void 0},{listings:r}=x?await D.getMobileListings(a):await D.getDesktopListings(a);R.recomendedWallets=Object.values(r)}return R.recomendedWallets},async getWallets(e){const t=((e,t)=>{for(var s in t||(t={}))S.call(t,s)&&N(e,s,t[s]);if(P)for(var s of P(t))_.call(t,s)&&N(e,s,t[s]);return e})({},e),{explorerRecommendedWalletIds:s,explorerExcludedWalletIds:o}=I.state,{recomendedWallets:n}=R;if("ALL"===o)return R.wallets;n.length?t.excludedIds=n.map((e=>e.id)).join(","):h.isArray(s)&&(t.excludedIds=s.join(",")),h.isArray(o)&&(t.excludedIds=[t.excludedIds,o].filter(Boolean).join(",")),v.state.isAuth&&(t.sdks="auth_v1");const{page:a,search:r}=e,{listings:i,total:l}=x?await D.getMobileListings(t):await D.getDesktopListings(t),c=Object.values(i),d=r?"search":"wallets";return R[d]={listings:[...R[d].listings,...c],total:l,page:a??1},{listings:c,total:l}},getWalletImageUrl:e=>D.getWalletImageUrl(e),getAssetImageUrl:e=>D.getAssetImageUrl(e),resetSearch(){R.search={listings:[],total:0,page:1}}},$=p({open:!1}),V={state:$,subscribe:e=>u($,(()=>e($))),open:async e=>new Promise((t=>{const{isUiLoaded:s,isDataLoaded:o}=v.state;if(h.removeWalletConnectDeepLink(),v.setWalletConnectUri(null==e?void 0:e.uri),v.setChains(null==e?void 0:e.chains),m.reset("ConnectWallet"),s&&o)$.open=!0,t();else{const e=setInterval((()=>{const s=v.state;s.isUiLoaded&&s.isDataLoaded&&(clearInterval(e),$.open=!0,t())}),200)}})),close(){$.open=!1}};var B=Object.defineProperty,H=Object.getOwnPropertySymbols,K=Object.prototype.hasOwnProperty,z=Object.prototype.propertyIsEnumerable,J=(e,t,s)=>t in e?B(e,t,{enumerable:!0,configurable:!0,writable:!0,value:s}):e[t]=s;const q=p({themeMode:typeof matchMedia<"u"&&matchMedia("(prefers-color-scheme: dark)").matches?"dark":"light"}),F={state:q,subscribe:e=>u(q,(()=>e(q))),setThemeConfig(e){const{themeMode:t,themeVariables:s}=e;t&&(q.themeMode=t),s&&(q.themeVariables=((e,t)=>{for(var s in t||(t={}))K.call(t,s)&&J(e,s,t[s]);if(H)for(var s of H(t))z.call(t,s)&&J(e,s,t[s]);return e})({},s))}},G=p({open:!1,message:"",variant:"success"}),Q={state:G,subscribe:e=>u(G,(()=>e(G))),openToast(e,t){G.open=!0,G.message=e,G.variant=t},closeToast(){G.open=!1}};const X=Object.freeze(Object.defineProperty({__proto__:null,WalletConnectModal:class{constructor(e){this.openModal=V.open,this.closeModal=V.close,this.subscribeModal=V.subscribe,this.setTheme=F.setThemeConfig,F.setThemeConfig(e),I.setConfig(e),this.initUi()}async initUi(){if(typeof window<"u"){await e((()=>import("./index-ZuaO7Tjw.js")),__vite__mapDeps([0,1,2]));const t=document.createElement("wcm-modal");document.body.insertAdjacentElement("beforeend",t),v.setIsUiLoaded(!0)}}}},Symbol.toStringTag,{value:"Module"}));export{b as R,m as T,h as a,X as i,F as n,Q as o,v as p,V as s,T as t,I as y};
