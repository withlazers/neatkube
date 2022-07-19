Neatkube
========

```
 /.     __     .\
| |_.-`/  \`-._| |
| |___/    \___| |
 \______________/
     Neatkube
```

The last kubernetes tool you'll ever need.

Kubernetes is a mess. Everthing ships it's own command line tools that you need
to install and track for updates. Everybody has their own set of scripts to
start a debug pod and a set of random tools to easy their lives.

Neatkube is an attempt to pack all these tools and helpers and give you unified
access to it.

Neatkube downloads tools on demand and is able to keep them up to date.

## Features

### 🧰 Toolbox

Neatkube includes many regulary used kubernetes tools, that will be downloaded
on demand:

* [🎮 kubectl](https://kubernetes.io/docs/reference/kubectl/kubectl/)
* [🪖 helm](https://helm.sh)
* [🗄️ helmfile](https://github.com/roboll/helmfile)
* [🎛️ k9s](https://k9scli.io/)
* [🔍 yq](https://github.com/mikefarah/yq)
* [🦭 kubeseal](https://sealed-secrets.netlify.app/)
* [📜 istio](https://istio.io/)
* [🔗 linkerd](https://linkerd.io/)
* [🧒 minikube](https://minikube.sigs.k8s.io/)
* [🌠 stern](https://github.com/stern/stern)

#### Example `helm`

```
nk helm install ...
```

#### Example `k9s`

```
nk 9 -n kube-system
```

### 🐚 Shell-in-a-pod

It's a common task to start a debug pod on kubernetes. *Neatkube* eases the
start and the configuration of such a debug pod.

#### Example simple shell

```
nk shell -n default
```

### 🧳 pack the configuration

*Neatkube* provides a small tool that reads a kubeconfig file and includes all
external resources. This is useful for `minikube` for example, that by default
puts its certificates on a different place on the file system.

#### Example cfgpack

```
nk cfgpack /path/to/kubeconfig
```
