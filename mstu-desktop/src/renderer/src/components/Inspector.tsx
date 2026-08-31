import { useState } from 'react'

import MappingRows from './MappingRows'
import type { Connector, Library, Mapping, Subscription } from '../types'

type Props = {
  connector: Connector | null
  source: Library | null
  target: Library | null
  onChangeMapping: (id: string, patch: Partial<Mapping>) => void
  onRemoveMapping: (id: string) => void
  onAddMapping: () => void
  onChangeSubscription: (subscriptionId: string, patch: Partial<Subscription>) => void
  onRemoveSubscription: (subscriptionId: string) => void
  onAddSubscription: () => void
  onChangeSubscriptionMapping: (
    subscriptionId: string,
    mappingId: string,
    patch: Partial<Mapping>
  ) => void
  onRemoveSubscriptionMapping: (subscriptionId: string, mappingId: string) => void
  onAddSubscriptionMapping: (subscriptionId: string) => void
}

type Tab = 'mapping' | 'events'

function Inspector(props: Props): JSX.Element {
  const { connector, source, target } = props
  const [tab, setTab] = useState<Tab>('mapping')

  if (!connector || !source || !target) {
    return (
      <section className="inspector">
        <div className="empty">
          Select a connector to map its fields,
          <br />
          or link two plugins to create one.
        </div>
      </section>
    )
  }

  return (
    <section className="inspector">
      <div className="inspector-head">
        <div className="inspector-route">
          <span>{source.name}</span>
          <span className="arrow">▸</span>
          <span>{target.name}</span>
        </div>
        <div className="inspector-route">
          <span className="id">
            node {connector.from} → node {connector.to}
          </span>
        </div>
      </div>

      <div className="tabs">
        <button
          className={`tab ${tab === 'mapping' ? 'active' : ''}`}
          onClick={() => setTab('mapping')}
        >
          Mapping<span className="count">{connector.mappings.length}</span>
        </button>
        <button
          className={`tab ${tab === 'events' ? 'active' : ''}`}
          onClick={() => setTab('events')}
        >
          Event subscribe<span className="count">{connector.subscriptions.length}</span>
        </button>
      </div>

      <div className="panel-body">
        {tab === 'mapping' ? (
          <>
            <div className="section-label">
              <span>process output → process input</span>
            </div>

            <MappingRows
              source={source.output?.fields ?? []}
              target={target.input?.fields ?? []}
              mappings={connector.mappings}
              onChange={props.onChangeMapping}
              onRemove={props.onRemoveMapping}
              onAdd={props.onAddMapping}
              addLabel="Add mapping"
            />
          </>
        ) : (
          <>
            <div className="section-label">
              <span>{source.name} events → {target.name} commands</span>
            </div>

            {connector.subscriptions.map((subscription) => {
              const event = source.events[subscription.event]
              const command = target.commands[subscription.command]

              return (
                <div className="subscription" key={subscription.id}>
                  <div className="subscription-head">
                    <span className="badge event">event</span>
                    <select
                      value={subscription.event}
                      onChange={(change) =>
                        props.onChangeSubscription(subscription.id, {
                          event: Number(change.target.value)
                        })
                      }
                    >
                      {source.events.map((item, index) => (
                        <option key={item.name} value={index}>
                          {item.name}
                        </option>
                      ))}
                    </select>

                    <span className="arrow">→</span>

                    <span className="badge command">cmd</span>
                    <select
                      value={subscription.command}
                      onChange={(change) =>
                        props.onChangeSubscription(subscription.id, {
                          command: Number(change.target.value)
                        })
                      }
                    >
                      {target.commands.map((item, index) => (
                        <option key={item.name} value={index}>
                          {item.name}
                        </option>
                      ))}
                    </select>

                    <span className="spacer" style={{ flex: 1 }} />

                    <button
                      className="remove"
                      title="Remove subscription"
                      onClick={() => props.onRemoveSubscription(subscription.id)}
                    >
                      ✕
                    </button>
                  </div>

                  <div className="subscription-body">
                    <MappingRows
                      source={event?.schema?.fields ?? []}
                      target={command?.schema?.fields ?? []}
                      mappings={subscription.mappings}
                      onChange={(id, patch) =>
                        props.onChangeSubscriptionMapping(subscription.id, id, patch)
                      }
                      onRemove={(id) => props.onRemoveSubscriptionMapping(subscription.id, id)}
                      onAdd={() => props.onAddSubscriptionMapping(subscription.id)}
                      addLabel="Add payload mapping"
                    />
                  </div>
                </div>
              )
            })}

            {connector.subscriptions.length === 0 && (
              <div className="empty">
                {source.name} publishes {source.events.length} event
                {source.events.length === 1 ? '' : 's'}. Subscribe one to a command on{' '}
                {target.name}.
              </div>
            )}

            <button
              className="add-row"
              onClick={props.onAddSubscription}
              disabled={source.events.length === 0 || target.commands.length === 0}
            >
              + Subscribe to an event
            </button>
          </>
        )}
      </div>
    </section>
  )
}

export default Inspector
