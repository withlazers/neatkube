Neatkube
========

[![Night Rider](https://upload.wikimedia.org/wikipedia/commons/thumb/3/33/Knight_Rider_Replica_1X7A8004.jpg/640px-Knight_Rider_Replica_1X7A8004.jpg)](https://de.m.wikipedia.org/wiki/Datei:Knight_Rider_Replica_1X7A8004.jpg)
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

*Neatkube* has two main objectives:

1. Give unified access to all most frequently used kubernetes tools and keep
   them up to date
2. Streamline common usage and debug patterns and mold them into a command
   line tools

In the end the goal is: If the *Neatkube* binary is installed on your system and
have access to a kubernetes cluster you can do something useful.

## Features

*Neatkube* features lots of different subcommands.

### 🐚 Shell-in-a-pod

It's a common task to start a debug pod on kubernetes. *Neatkube* eases the
start and the configuration of such a debug pod.

#### Example simple shell

```
nk shell -n default
```

#### Access a certain Node in a cluster

```
nk shell -pNIP --node "MYNODE" chroot /bin/sh
```

### 🧳 pack the configuration

*Neatkube* provides a small tool that reads a kubeconfig file and includes all
external resources. This is useful for `minikube` for example, that by default
puts its certificates on a different place on the file system.

#### Example cfgpack

```
nk cfgpack /path/to/kubeconfig
```

### 🧰 Toolbox

*Neatkube* includes many regulary used kubernetes tools, that will be downloaded
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
nk k9s -n kube-system
```

