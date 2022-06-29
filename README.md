Neatkube
========

The last kubernetes tool you'll ever need.

Kubernetes is a mess. Everthing ships it's own command line tools that you need
to install and track for updates. Everybody has their own set of scripts to
start a debug pod and a set of random tools to easy their lives.

Neatkube is an attempt to pack all these tools and helpers and give you unified
access to it.

Neatkube downloads tools on demand and is able to keep them up to date.

### Usage examples:

#### start k9s:

```
nk
```

Neatkube by default calls k9s if there are no arguments present.
You may even define options that will be passed to k9s:

```
k9s -n kube-system
```

#### call kubectl to get all pods of a cluster

```
nk get pods -A
```

#### install a helm chart

```
nk helm install bitnami/wordpress awesomeblog
```

#### Start a new pod and open a shell with a hostmount and a specific serviceaccount

```
nk shell -a my-service-account -H /:/my-host
```

* `/my-host` is optional, by default a hostmount will be mounted as `/host`

#### Spit out a dereferenced version of your kubeconfig

```
nk cfgpack
```

This kubeconfig has no dependencies to other files and can be moved for examples
to other hosts.

#### List all available tools

By default `nk help` only lists tools that are already present locally. The
command below lists all available tools, not included the builtin helpers
such as `nk shell`:

```
nk toolbox list
```
